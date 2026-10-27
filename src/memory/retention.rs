use super::{Store, now};
use anyhow::ensure;

const MIB: u64 = 1024 * 1024;
impl Store {
    fn wal_bytes(&self) -> u64 {
        std::fs::metadata(format!("{}-wal", self.path.display())).map_or(0, |m| m.len())
    }
    pub fn disk_bytes(&self) -> u64 {
        [
            self.path.clone(),
            format!("{}-wal", self.path.display()).into(),
            format!("{}-shm", self.path.display()).into(),
        ]
        .iter()
        .map(|path| std::fs::metadata(path).map_or(0, |m| m.len()))
        .sum()
    }

    pub(super) fn write_ready(&self) -> anyhow::Result<()> {
        if self.wal_bytes() >= 16 * MIB {
            let _: (i64, i64, i64) =
                self.db
                    .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })?;
            if self.wal_bytes() >= 32 * MIB {
                let (busy, _, _): (i64, i64, i64) =
                    self.db
                        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                        })?;
                ensure!(
                    busy == 0 && self.wal_bytes() < 32 * MIB,
                    "memory writes paused: WAL checkpoint is blocked"
                );
            }
        }
        ensure!(
            self.disk_bytes() < 256 * MIB,
            "memory writes paused: storage budget reached"
        );
        Ok(())
    }

    pub(super) fn memory_room(&self, extra: i64) -> anyhow::Result<()> {
        let bytes:i64=self.db.query_row("SELECT (SELECT coalesce(sum(length(CAST(body AS BLOB))+256),0) FROM memories)+(SELECT coalesce(sum(length(CAST(excerpt AS BLOB))+128),0) FROM sources)+(SELECT coalesce(sum(length(CAST(body AS BLOB))+128),0) FROM versions)",[],|r|r.get(0))?;
        ensure!(
            bytes + extra < 16 * 1024 * 1024,
            "retained memories are full; remove an old memory first"
        );
        Ok(())
    }

    pub(super) fn admit(&mut self) -> anyhow::Result<()> {
        self.write_ready()?;
        self.maintain(false)?;
        let page: i64 = self.db.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let pages: i64 = self.db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let free: i64 = self
            .db
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        if (pages - free) * page >= (128 * MIB) as i64 {
            self.maintain(true)?;
        }
        let pages: i64 = self.db.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let free: i64 = self
            .db
            .query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
        ensure!(
            (pages - free) * page < (160 * MIB) as i64,
            "memory writes paused: retained data is full"
        );
        Ok(())
    }

    pub fn maintain(&mut self, force: bool) -> anyhow::Result<()> {
        if !force && now() - self.maintenance < 10 {
            return Ok(());
        }
        self.write_ready()?;
        let at = now();
        self.db.execute(
            "DELETE FROM events WHERE id IN(SELECT id FROM events WHERE at<?1 LIMIT 500)",
            [at - 7 * 86400],
        )?;
        self.db.execute(
            "DELETE FROM suggestions WHERE id IN(SELECT id FROM suggestions WHERE at<?1 LIMIT 500)",
            [at - 30 * 86400],
        )?;
        self.db
            .execute("DELETE FROM recalls WHERE at<?1", [at - 7 * 86400])?;
        self.db
            .execute("DELETE FROM jobs WHERE at<?1", [at - 7 * 86400])?;
        self.db
            .execute("DELETE FROM attempts WHERE at<?1", [at - 3600])?;
        self.trim("events", "body", 32 * MIB, 16384)?;
        self.trim("suggestions", "body", 8 * MIB, 4096)?;
        self.db.execute("DELETE FROM recalls WHERE id IN(SELECT id FROM recalls ORDER BY at DESC LIMIT -1 OFFSET 4096)",[])?;
        self.db.execute("DELETE FROM events WHERE id IN(SELECT id FROM (SELECT id,row_number() OVER(PARTITION BY project ORDER BY at DESC,rowid DESC) AS rank FROM events) WHERE rank>1024 LIMIT 500)",[])?;
        self.db.execute("DELETE FROM projects WHERE seen<?1 AND id NOT IN(SELECT project FROM memories WHERE project IS NOT NULL) AND id NOT IN(SELECT project FROM events) AND id NOT IN(SELECT project FROM suggestions) AND id NOT IN(SELECT project FROM jobs)",[at-30*86400])?;
        self.db.execute_batch("PRAGMA incremental_vacuum(128);")?;
        let _: (i64, i64, i64) = self
            .db
            .query_row("PRAGMA wal_checkpoint(PASSIVE)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        self.maintenance = at;
        Ok(())
    }

    pub(super) fn trim(
        &self,
        table: &str,
        column: &str,
        budget: u64,
        count: u64,
    ) -> anyhow::Result<()> {
        let (bytes, rows): (i64, i64) = self.db.query_row(
            &format!(
                "SELECT coalesce(sum(length(CAST({column} AS BLOB))+256),0),count(*) FROM {table}"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if bytes > budget as i64 || rows > count as i64 {
            self.db.execute(&format!("DELETE FROM {table} WHERE id IN(SELECT id FROM {table} ORDER BY at ASC LIMIT 500)"),[])?;
            log::debug!("memory: pruned bounded batch from {table}");
        }
        Ok(())
    }
}
