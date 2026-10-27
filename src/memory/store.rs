use super::{bounded, clean, key, now};
use anyhow::{Context as _, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub struct Store {
    pub(super) db: Connection,
    pub(super) path: PathBuf,
    pub(super) maintenance: i64,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub kind: String,
    pub excerpt: String,
    pub at: i64,
    pub retained: bool,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub id: String,
    pub project: Option<String>,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub revision: i64,
    pub can_undo: bool,
    pub sources: Vec<Source>,
}
#[derive(Default, Clone)]
pub struct Overview {
    pub items: Vec<Item>,
    pub bytes: u64,
    pub project: Option<String>,
    pub projects: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub text: String,
}
#[derive(Clone, Serialize)]
pub struct Job {
    pub checkout: String,
    pub project: String,
    pub session: String,
    pub revision: i64,
    pub evidence: Vec<Evidence>,
}

impl Store {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(&path)?;
        db.busy_timeout(Duration::from_millis(200))?;
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA wal_autocheckpoint=256; PRAGMA journal_size_limit=4194304;")?;
        let version: u32 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        ensure!(
            version <= 1,
            "memory database was created by a newer version"
        );
        let page: i64 = db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let limit: i64 = db.query_row(
            &format!("PRAGMA max_page_count={}", 192 * 1024 * 1024 / page),
            [],
            |r| r.get(0),
        )?;
        ensure!(
            limit * page <= 192 * 1024 * 1024,
            "memory database exceeds its size limit"
        );
        db.execute_batch(include_str!("schema.sql"))?;
        db.execute("UPDATE jobs SET state='pending' WHERE state='running'", [])?;
        let mut store = Self {
            db,
            path,
            maintenance: 0,
        };
        store.maintain(true)?;
        log::info!("memory: database ready with a 192 MiB main-file limit");
        Ok(store)
    }

    pub fn project(&mut self, folder: &Path) -> anyhow::Result<String> {
        let absolute = folder
            .canonicalize()
            .unwrap_or_else(|_| folder.to_path_buf());
        let common = std::process::Command::new("git")
            .args(["-C"])
            .arg(&absolute)
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|path| PathBuf::from(path.trim()))
            .and_then(|path| path.canonicalize().ok())
            .unwrap_or(absolute);
        let id = key(&common.to_string_lossy());
        let exists: bool = self.db.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            params![id],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(id);
        }
        self.write_ready()?;
        let count: i64 = self.db.query_row(
            "SELECT count(*) FROM projects WHERE id!=?1",
            params![id],
            |r| r.get(0),
        )?;
        ensure!(count < 256, "project memory catalog is full");
        self.db.execute("INSERT INTO projects VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET seen=excluded.seen", params![id, bounded(&folder.to_string_lossy(),1024),now()])?;
        Ok(id)
    }

    pub fn record(
        &mut self,
        folder: &Path,
        session: &str,
        kind: &str,
        body: &str,
    ) -> anyhow::Result<()> {
        ensure!(
            ["prompt", "reported", "failed", "question"].contains(&kind),
            "unknown event kind"
        );
        let body = clean(body);
        if body.is_empty() {
            return Ok(());
        }
        self.admit()?;
        let project = self.project(folder)?;
        let id = uuid::Uuid::new_v4().to_string();
        let tx = self.db.transaction()?;
        tx.execute(
            "INSERT INTO events VALUES(?1,?2,?3,?4,?5,?6)",
            params![id, project, session, kind, body, now()],
        )?;
        if kind != "prompt" {
            tx.execute("INSERT INTO jobs(project,session,at,checkout) VALUES(?1,?2,?3,?4) ON CONFLICT(project) DO UPDATE SET session=excluded.session,state='pending',at=excluded.at,checkout=excluded.checkout",params![project,session,now(),super::checkout(folder)])?;
        }
        tx.execute("DELETE FROM events WHERE rowid IN(SELECT rowid FROM events ORDER BY at,rowid LIMIT max((SELECT count(*) FROM events)-14000,0))",[])?;
        tx.execute("DELETE FROM events WHERE rowid IN(SELECT rowid FROM events WHERE project=?1 ORDER BY at,rowid LIMIT max((SELECT count(*) FROM events WHERE project=?1)-900,0))",params![project])?;
        tx.commit()?;
        self.maintain(false)
    }

    pub fn save(
        &mut self,
        folder: Option<&Path>,
        kind: &str,
        body: &str,
        edit: Option<(&str, i64)>,
        source: Option<&str>,
    ) -> anyhow::Result<()> {
        ensure!(
            ["preference", "note", "unresolved", "decision"].contains(&kind),
            "unknown memory kind"
        );
        ensure!(
            (kind == "preference") == folder.is_none(),
            "preferences are global; project notes need a project"
        );
        let body = clean(body);
        ensure!(
            !body.is_empty() && body.len() <= 1024,
            "memory must be 1–1024 UTF-8 bytes without credential lines"
        );
        self.admit()?;
        self.memory_room(4096)?;
        let total: i64 = self
            .db
            .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))?;
        ensure!(total < 4000 || edit.is_some(), "memory item budget reached");
        let project = folder.map(|folder| self.project(folder)).transpose()?;
        let id = edit
            .map(|(id, _)| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let tx = self.db.transaction()?;
        if let Some((_, revision)) = edit {
            tx.execute(
                "DELETE FROM events WHERE id IN(SELECT event FROM sources WHERE memory=?1)",
                params![id],
            )?;
            tx.execute(
                "DELETE FROM jobs WHERE project IN(SELECT project FROM memories WHERE id=?1)",
                params![id],
            )?;
            tx.execute("INSERT INTO versions SELECT id,revision,body,updated FROM memories WHERE id=?1 AND revision=?2",params![id,revision])?;
            tx.execute("DELETE FROM versions WHERE memory=?1 AND revision NOT IN(SELECT revision FROM versions WHERE memory=?1 ORDER BY revision DESC LIMIT 3)",params![id])?;
            ensure!(tx.execute("UPDATE memories SET body=?1,updated=?2,revision=revision+1,status='explicit' WHERE id=?3 AND revision=?4",params![body,now(),id,revision])? == 1,"memory changed; refresh before editing");
        } else {
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM memories WHERE project IS ?1 AND status!='superseded'",
                params![project],
                |r| r.get(0),
            )?;
            ensure!(
                count < if project.is_none() { 100 } else { 300 },
                "memory scope is full; remove an old memory first"
            );
            tx.execute("INSERT INTO memories(id,project,kind,body,status,at,updated) VALUES(?1,?2,?3,?4,'explicit',?5,?5)", params![id,project,kind,body,now()])?;
        }
        let event = uuid::Uuid::new_v4().to_string();
        tx.execute("DELETE FROM sources WHERE memory=?1", params![id])?;
        tx.execute(
            "INSERT INTO sources VALUES(?1,?2,'user',?3,?4)",
            params![
                id,
                event,
                bounded(&clean(source.unwrap_or(&body)), 512),
                now()
            ],
        )?;
        tx.execute("UPDATE meta SET value=value+1 WHERE key='revision'", [])?;
        tx.execute("DELETE FROM recalls WHERE memory=?1", params![id])?;
        tx.commit()?;
        log::info!("memory: explicit memory saved");
        Ok(())
    }

    pub fn forget(&mut self, id: &str) -> anyhow::Result<()> {
        self.write_ready()?;
        let tx = self.db.transaction()?;
        let project: Option<String> = tx
            .query_row(
                "SELECT project FROM memories WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        tx.execute(
            "DELETE FROM events WHERE id IN (SELECT event FROM sources WHERE memory=?1)",
            params![id],
        )?;
        tx.execute("DELETE FROM jobs WHERE project IS ?1", params![project])?;
        tx.execute("DELETE FROM memories WHERE id IN(SELECT a.memory FROM sources a JOIN sources b ON a.event=b.event WHERE b.memory=?1) OR id=?1", params![id])?;
        tx.execute("UPDATE meta SET value=value+1 WHERE key='revision'", [])?;
        tx.commit()?;
        self.maintain(true)?;
        log::info!("memory: memory and dependent history removed");
        Ok(())
    }

    pub fn clear(&mut self) -> anyhow::Result<()> {
        self.write_ready()?;
        self.db.execute_batch("BEGIN; DELETE FROM memories; DELETE FROM events; DELETE FROM suggestions; DELETE FROM jobs; DELETE FROM projects; UPDATE meta SET value=value+1 WHERE key='revision'; COMMIT;")?;
        self.maintain(true)?;
        log::info!("memory: all stored memories cleared");
        Ok(())
    }

    pub fn overview(&mut self, folder: Option<&Path>, query: &str) -> anyhow::Result<Overview> {
        let project = folder.map(|folder| self.project(folder)).transpose()?;
        let mut statement = self.db.prepare("SELECT id,project,kind,body,status,revision,EXISTS(SELECT 1 FROM versions WHERE memory=memories.id) FROM memories WHERE (project IS NULL OR project=?1) AND status!='superseded' ORDER BY updated DESC LIMIT 400")?;
        let items = statement
            .query_map(params![project], |r| {
                Ok(Item {
                    id: r.get(0)?,
                    project: r.get(1)?,
                    kind: r.get(2)?,
                    body: r.get(3)?,
                    status: r.get(4)?,
                    revision: r.get(5)?,
                    can_undo: r.get(6)?,
                    sources: vec![],
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let query = query.trim().to_lowercase();
        let indexed = if query.chars().count() >= 3 {
            let expression = format!("\"{}\"", query.replace('"', "\"\""));
            Some(self.db.prepare("SELECT m.id FROM memory_search JOIN memories m ON m.rowid=memory_search.rowid WHERE memory_search MATCH ?1 AND (m.project IS NULL OR m.project=?2) LIMIT 400")?.query_map(params![expression,project],|r|r.get::<_,String>(0))?.collect::<Result<std::collections::HashSet<_>,_>>()?)
        } else {
            None
        };
        let mut found = Vec::new();
        for mut item in items
            .into_iter()
            .filter(|item| {
                query.is_empty()
                    || indexed.as_ref().map_or_else(
                        || item.body.to_lowercase().contains(&query),
                        |ids| ids.contains(&item.id),
                    )
            })
            .take(60)
        {
            item.sources = self.db.prepare("SELECT s.kind,s.excerpt,s.at,EXISTS(SELECT 1 FROM events e WHERE e.id=s.event) FROM sources s WHERE memory=?1 LIMIT 6")?.query_map(params![item.id],|r|Ok(Source {kind:r.get(0)?,excerpt:r.get(1)?,at:r.get(2)?,retained:r.get(3)?}))?.collect::<Result<Vec<_>,_>>()?;
            found.push(item);
        }
        let projects = self
            .db
            .prepare("SELECT path FROM projects ORDER BY seen DESC LIMIT 256")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Overview {
            items: found,
            bytes: self.disk_bytes(),
            project,
            projects,
        })
    }

    pub fn undo(&mut self, id: &str, revision: i64) -> anyhow::Result<()> {
        self.write_ready()?;
        let body: String = self.db.query_row(
            "SELECT body FROM versions WHERE memory=?1 ORDER BY revision DESC LIMIT 1",
            params![id],
            |r| r.get(0),
        )?;
        let tx = self.db.transaction()?;
        ensure!(tx.execute("UPDATE memories SET body=?1,updated=?2,revision=revision+1 WHERE id=?3 AND revision=?4",params![body,now(),id,revision])?==1,"memory changed; refresh before undoing");
        tx.execute("DELETE FROM versions WHERE memory=?1 AND revision=(SELECT max(revision) FROM versions WHERE memory=?1)",params![id])?;
        tx.execute("DELETE FROM sources WHERE memory=?1", params![id])?;
        tx.execute(
            "INSERT INTO sources VALUES(?1,?2,'user',?3,?4)",
            params![
                id,
                uuid::Uuid::new_v4().to_string(),
                bounded(&body, 512),
                now()
            ],
        )?;
        tx.execute("UPDATE meta SET value=value+1 WHERE key='revision'", [])?;
        tx.execute("DELETE FROM recalls WHERE memory=?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn revision(&self) -> anyhow::Result<i64> {
        self.db
            .query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                r.get(0)
            })
            .context("memory revision")
    }
}
