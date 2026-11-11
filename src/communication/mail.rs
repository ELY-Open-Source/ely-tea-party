use std::path::Path;

use anyhow::{Context, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub from: String,
    pub to: String,
    pub body: String,
    pub reply_to: Option<String>,
    pub created_at: i64,
    pub acknowledged: bool,
}

pub struct Mail(Connection);

impl Mail {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .open(path)?;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        Self::new(Connection::open(path).context("messages: open local mailbox")?)
    }

    fn new(db: Connection) -> anyhow::Result<Self> {
        db.busy_timeout(std::time::Duration::from_secs(2))?;
        db.execute_batch("CREATE TABLE IF NOT EXISTS mail (
            id TEXT PRIMARY KEY, sender TEXT NOT NULL, recipient TEXT NOT NULL,
            body TEXT NOT NULL, reply_to TEXT, created_at INTEGER NOT NULL,
            acknowledged INTEGER NOT NULL DEFAULT 0, request_key TEXT NOT NULL,
            UNIQUE(sender, request_key));
            CREATE INDEX IF NOT EXISTS mail_recipient ON mail(recipient, acknowledged, created_at);")?;
        Ok(Self(db))
    }

    pub fn send(
        &self,
        from: &str,
        to: &str,
        body: &str,
        reply: Option<&str>,
        key: &str,
    ) -> anyhow::Result<Message> {
        ensure!(
            !body.trim().is_empty() && body.len() <= 32 * 1024,
            "messages: body must be 1–32768 bytes"
        );
        uuid::Uuid::parse_str(key).context("messages: --key must be a UUID")?;
        if let Some(old) = self.retry(from, key)? {
            ensure!(
                old.to == to && old.body == body && old.reply_to.as_deref() == reply,
                "messages: request key was already used for different content"
            );
            return Ok(old);
        }
        let message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            from: from.into(),
            to: to.into(),
            body: body.into(),
            reply_to: reply.map(str::to_owned),
            created_at: chrono::Utc::now().timestamp_millis(),
            acknowledged: false,
        };
        self.0.execute("INSERT INTO mail(id,sender,recipient,body,reply_to,created_at,request_key) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![message.id, from, to, body, reply, message.created_at, key])?;
        Ok(message)
    }

    pub fn retry(&self, from: &str, key: &str) -> anyhow::Result<Option<Message>> {
        Ok(self.0.query_row("SELECT id,sender,recipient,body,reply_to,created_at,acknowledged FROM mail WHERE sender=?1 AND request_key=?2",
            params![from, key], read).optional()?)
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Message> {
        self.0.query_row("SELECT id,sender,recipient,body,reply_to,created_at,acknowledged FROM mail WHERE id=?1", [id], read)
            .context("messages: message not found")
    }

    pub fn inbox(&self, address: &str, all: bool) -> anyhow::Result<Vec<Message>> {
        let mut query = self.0.prepare("SELECT id,sender,recipient,body,reply_to,created_at,acknowledged FROM mail
            WHERE recipient=?1 AND (?2 OR acknowledged=0) ORDER BY created_at DESC,rowid DESC LIMIT 100")?;
        Ok(query
            .query_map(params![address, all], read)?
            .collect::<Result<_, _>>()?)
    }

    pub fn unread(&self, address: &str) -> anyhow::Result<u64> {
        Ok(self
            .0
            .query_row(
                "SELECT COUNT(*) FROM mail WHERE recipient=?1 AND acknowledged=0",
                [address],
                |row| row.get::<_, i64>(0),
            )?
            .try_into()?)
    }

    pub fn ack(&self, address: &str, id: &str) -> anyhow::Result<()> {
        let changed = self.0.execute(
            "UPDATE mail SET acknowledged=1 WHERE id=?1 AND recipient=?2",
            params![id, address],
        )?;
        ensure!(
            changed == 1,
            "messages: only the recipient can acknowledge this message"
        );
        Ok(())
    }
}

fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        from: row.get(1)?,
        to: row.get(2)?,
        body: row.get(3)?,
        reply_to: row.get(4)?,
        created_at: row.get(5)?,
        acknowledged: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_is_scoped_acknowledged_explicitly_and_idempotent() {
        let mail = Mail::new(Connection::open_in_memory().unwrap()).unwrap();
        let key = uuid::Uuid::new_v4().to_string();
        let msg = mail.send("a", "b", "hello", None, &key).unwrap();
        assert_eq!(mail.send("a", "b", "hello", None, &key).unwrap().id, msg.id);
        assert!(mail.send("a", "c", "hello", None, &key).is_err());
        assert_eq!(mail.inbox("b", false).unwrap().len(), 1);
        assert_eq!(mail.unread("b").unwrap(), 1);
        assert!(mail.inbox("c", true).unwrap().is_empty());
        assert!(mail.ack("c", &msg.id).is_err());
        mail.ack("b", &msg.id).unwrap();
        assert!(mail.inbox("b", false).unwrap().is_empty());
        assert!(mail.inbox("b", true).unwrap()[0].acknowledged);
    }

    #[test]
    fn stored_mail_survives_reopen_without_moving_to_new_instances() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let path = dir.join("mail.db");
        Mail::open(&path)
            .unwrap()
            .send(
                "a",
                "old-b",
                "hello",
                None,
                &uuid::Uuid::new_v4().to_string(),
            )
            .unwrap();
        let mail = Mail::open(&path).unwrap();
        assert_eq!(mail.inbox("old-b", true).unwrap().len(), 1);
        assert!(mail.inbox("new-b", true).unwrap().is_empty());
        drop(mail);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
