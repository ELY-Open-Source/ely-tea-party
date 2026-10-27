use super::{Job, Store, bounded, clean, key, now, store::Evidence};
use anyhow::ensure;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    memories: Vec<Candidate>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    kind: String,
    text: String,
    sources: Vec<String>,
}

impl Store {
    pub fn claim(&mut self, folder: &std::path::Path) -> anyhow::Result<Option<Job>> {
        self.admit()?;
        let project = self.project(folder)?;
        let count: i64 = self.db.query_row(
            "SELECT count(*) FROM attempts WHERE at>?1",
            [now() - 3600],
            |r| r.get(0),
        )?;
        let recent: i64 = self.db.query_row(
            "SELECT count(*) FROM attempts WHERE at>?1",
            [now() - 1800],
            |r| r.get(0),
        )?;
        if count >= 2 || recent > 0 {
            return Ok(None);
        }
        let session: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT session,checkout FROM jobs WHERE project=?1 AND state='pending'",
                params![project],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((session, checkout)) = session else {
            return Ok(None);
        };
        let evidence=self.db.prepare("SELECT id,kind,body FROM events WHERE project=?1 AND session=?2 ORDER BY at DESC,rowid DESC LIMIT 4")?
            .query_map(params![project,session],|r|Ok(Evidence{id:r.get(0)?,kind:r.get(1)?,text:r.get(2)?}))?.collect::<Result<Vec<_>,_>>()?;
        if evidence.is_empty() {
            self.db
                .execute("DELETE FROM jobs WHERE project=?1", params![project])?;
            return Ok(None);
        }
        let revision = self.revision()?;
        let tx = self.db.transaction()?;
        tx.execute(
            "UPDATE jobs SET state='running',revision=?1 WHERE project=?2",
            params![revision, project],
        )?;
        tx.execute("INSERT INTO attempts VALUES(?1)", [now()])?;
        tx.commit()?;
        Ok(Some(Job {
            checkout,
            project,
            session,
            revision,
            evidence,
        }))
    }

    pub fn finish(&mut self, job: &Job, json: &str) -> anyhow::Result<()> {
        self.write_ready()?;
        self.memory_room(4608)?;
        ensure!(json.len() <= 8192, "memory proposal is too large");
        let update: Update = serde_json::from_str(json)?;
        ensure!(update.memories.len() <= 2, "too many memory proposals");
        ensure!(
            self.revision()? == job.revision,
            "memory changed while proposal was being prepared"
        );
        let expected:bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE project=?1 AND session=?2 AND state='running' AND revision=?3)",params![job.project,job.session,job.revision],|r|r.get(0))?;
        ensure!(expected, "memory job was superseded");
        let count: i64 = self
            .db
            .query_row("SELECT count(*) FROM memories", [], |r| r.get(0))?;
        ensure!(
            count + update.memories.len() as i64 <= 4000,
            "memory item budget reached"
        );
        let task = job
            .evidence
            .iter()
            .find(|e| e.kind == "prompt")
            .map(|e| key(&format!("{}\n{}", job.checkout, super::task_key(&e.text))))
            .unwrap_or_default();
        for candidate in &update.memories {
            ensure!(
                ["note", "unresolved", "decision"].contains(&candidate.kind.as_str()),
                "model cannot write preferences"
            );
            ensure!(
                !candidate.text.trim().is_empty()
                    && candidate.text.len() <= 600
                    && clean(&candidate.text) == candidate.text.trim(),
                "invalid memory proposal text"
            );
            ensure!(
                !candidate.sources.is_empty()
                    && candidate.sources.len() <= 3
                    && candidate
                        .sources
                        .iter()
                        .all(|id| job.evidence.iter().any(|e| e.id == *id)),
                "proposal contains unknown sources"
            );
            if candidate.kind == "decision" {
                ensure!(
                    candidate.sources.iter().any(|id| job
                        .evidence
                        .iter()
                        .any(|e| e.id == *id && e.kind == "prompt")),
                    "decisions require user evidence"
                );
            }
        }
        let tx = self.db.transaction()?;
        for candidate in update.memories {
            let existing: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM memories WHERE project=?1 AND body=?2)",
                params![job.project, candidate.text],
                |r| r.get(0),
            )?;
            if existing {
                continue;
            }
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM memories WHERE project=?1",
                params![job.project],
                |r| r.get(0),
            )?;
            ensure!(count < 300, "project memory budget reached");
            let id = uuid::Uuid::new_v4().to_string();
            tx.execute("INSERT INTO memories(id,project,kind,body,task,status,at,updated) VALUES(?1,?2,?3,?4,?5,'reported',?6,?6)",params![id,job.project,candidate.kind,candidate.text,task,now()])?;
            for source in candidate.sources {
                let evidence = job
                    .evidence
                    .iter()
                    .find(|e| e.id == source)
                    .expect("validated source");
                let at: i64 =
                    tx.query_row("SELECT at FROM events WHERE id=?1", params![source], |r| {
                        r.get(0)
                    })?;
                tx.execute(
                    "INSERT INTO sources VALUES(?1,?2,?3,?4,?5)",
                    params![id, source, evidence.kind, bounded(&evidence.text, 512), at],
                )?;
            }
        }
        tx.execute("DELETE FROM jobs WHERE project=?1", params![job.project])?;
        tx.commit()?;
        log::info!("memory: bounded project proposals stored with evidence");
        Ok(())
    }

    pub fn fail(&mut self, job: &Job) -> anyhow::Result<()> {
        self.db.execute(
            "UPDATE jobs SET state='failed' WHERE project=?1 AND revision=?2 AND state='running'",
            params![job.project, job.revision],
        )?;
        Ok(())
    }
}
