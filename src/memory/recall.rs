use super::{Context, Recall, Store, bounded, key, now};
use anyhow::ensure;
use rusqlite::params;
use std::path::Path;

impl Store {
    pub fn recall(
        &mut self,
        folder: &Path,
        session: &str,
        task: &str,
        revision: u64,
    ) -> anyhow::Result<Context> {
        let project = self.project(folder)?;
        let preferences=self.db.prepare("SELECT body FROM memories WHERE project IS NULL AND kind='preference' AND status='explicit' ORDER BY updated DESC,rowid DESC LIMIT 3")?
            .query_map([],|r|r.get::<_,String>(0))?.map(|r|r.map(|s|bounded(&s,200))).collect::<Result<Vec<_>,_>>()?;
        let mut result = Context {
            revision,
            preferences,
            recalled: vec![],
        };
        if task.trim().is_empty() {
            return Ok(result);
        }
        let task = key(&format!(
            "{}\n{}",
            super::checkout(folder),
            super::task_key(task)
        ));
        let rows=self.db.prepare("SELECT m.id,m.body FROM memories m WHERE m.project=?1 AND m.kind='unresolved' AND m.task=?2 AND m.status='reported' AND m.updated>?3 AND NOT EXISTS(SELECT 1 FROM recalls r WHERE r.memory=m.id AND r.session=?4 AND r.task=?2) ORDER BY m.updated DESC LIMIT 2")?
            .query_map(params![project,task,now()-30*86400,session],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<Result<Vec<_>,_>>()?;
        for (id, body) in rows {
            result.recalled.push(Recall {
                id,
                text: bounded(&body, 300),
                reason: "same_project_same_unresolved_task".into(),
            });
        }
        Ok(result)
    }

    pub fn used(&mut self, session: &str, task: &str, context: &Context) -> anyhow::Result<()> {
        self.write_ready()?;
        for item in &context.recalled {
            self.db.execute("INSERT INTO recalls(memory,session,task,reason,at) SELECT id,?1,task,?3,?4 FROM memories WHERE id=?5",params![session,super::task_key(task),item.reason,now(),item.id])?;
        }
        Ok(())
    }

    pub fn suggestion(&mut self, folder: &Path, text: &str) -> anyhow::Result<Option<String>> {
        self.admit()?;
        let project = self.project(folder)?;
        let text = super::clean(text);
        if text.is_empty() {
            return Ok(None);
        }
        let previous=self.db.prepare("SELECT body FROM suggestions WHERE project=?1 AND at>?2 ORDER BY at DESC LIMIT 100")?
            .query_map(params![project,now()-30*86400],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        if previous.iter().any(|old| similar(old, &text)) {
            return Ok(None);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.db.execute(
            "INSERT INTO suggestions(id,project,body,at) VALUES(?1,?2,?3,?4)",
            params![id, project, text, now()],
        )?;
        Ok(Some(id))
    }

    pub fn reject(&mut self, id: &str) -> anyhow::Result<()> {
        self.write_ready()?;
        ensure!(
            self.db.execute(
                "UPDATE suggestions SET feedback='rejected' WHERE id=?1",
                params![id]
            )? == 1,
            "suggestion no longer exists"
        );
        log::info!("memory: suggestion explicitly rejected");
        Ok(())
    }
}

fn similar(a: &str, b: &str) -> bool {
    let grams = |s: &str| {
        let chars = s
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<Vec<_>>();
        chars
            .windows(2)
            .map(|s| s.iter().collect::<String>())
            .collect::<std::collections::HashSet<_>>()
    };
    let a = grams(a);
    let b = grams(b);
    !a.is_empty() && a.intersection(&b).count() * 100 >= a.union(&b).count() * 75
}
