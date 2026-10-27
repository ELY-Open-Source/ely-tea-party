use super::*;
use rusqlite::params;
struct Fixture {
    dir: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ely-memory-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::create_dir_all(dir.join("b")).unwrap();
        Self { dir }
    }
    fn store(&self) -> Store {
        Store::open(self.dir.join("memory.sqlite")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn explicit_corrections_survive_restart_and_forgetting_updates_search() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store
        .save(None, "preference", "直接聊项目，不要固定开场。", None, None)
        .unwrap();
    let item = store.overview(None, "").unwrap().items.remove(0);
    store
        .save(
            None,
            "preference",
            "自然简短，不要刻意卖萌。",
            Some((&item.id, item.revision)),
            None,
        )
        .unwrap();
    assert!(
        store
            .save(
                None,
                "preference",
                "stale",
                Some((&item.id, item.revision)),
                None
            )
            .is_err()
    );
    drop(store);
    let mut store = fixture.store();
    let context = store
        .recall(&fixture.dir.join("a"), "session", "new task", 0)
        .unwrap();
    assert_eq!(context.preferences, vec!["自然简短，不要刻意卖萌。"]);
    assert!(context.recalled.is_empty());
    store.forget(&item.id).unwrap();
    assert!(store.overview(None, "卖萌").unwrap().items.is_empty());
    assert!(
        store
            .recall(&fixture.dir.join("a"), "session", "new task", 0)
            .unwrap()
            .preferences
            .is_empty()
    );
}

#[test]
fn ordinary_remarks_do_not_recall_history_and_related_tasks_stay_in_scope() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let a = fixture.dir.join("a");
    let b = fixture.dir.join("b");
    store.record(&a, "old", "prompt", "修复登录").unwrap();
    store.record(&a, "old", "failed", "等待测试库配置").unwrap();
    let job = store.claim(&a).unwrap().unwrap();
    let source = job
        .evidence
        .iter()
        .find(|e| e.kind == "failed")
        .unwrap()
        .id
        .clone();
    store.finish(&job,&serde_json::json!({"memories":[{"kind":"unresolved","text":"Agent 汇报：等待测试库配置。","sources":[source]}]}).to_string()).unwrap();
    assert!(
        store
            .recall(&a, "new", "设计下载页", 0)
            .unwrap()
            .recalled
            .is_empty()
    );
    assert!(
        store
            .recall(&b, "new", "修复登录", 0)
            .unwrap()
            .recalled
            .is_empty()
    );
    let context = store.recall(&a, "new", "修复登录", 0).unwrap();
    assert_eq!(context.recalled.len(), 1);
    store.used("new", "修复登录", &context).unwrap();
    assert!(
        store
            .recall(&a, "new", "修复登录", 0)
            .unwrap()
            .recalled
            .is_empty()
    );
}

#[test]
fn model_cannot_invent_sources_or_write_preferences_and_stale_jobs_cannot_resurrect_memory() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let a = fixture.dir.join("a");
    store
        .record(&a, "session", "prompt", "检查下载入口")
        .unwrap();
    store
        .record(&a, "session", "reported", "Agent 汇报已更新")
        .unwrap();
    let job = store.claim(&a).unwrap().unwrap();
    assert!(
        store
            .finish(
                &job,
                r#"{"memories":[{"kind":"note","text":"invented","sources":["missing"]}]}"#
            )
            .is_err()
    );
    assert!(store.finish(&job,&serde_json::json!({"memories":[{"kind":"preference","text":"likes jokes","sources":[job.evidence[0].id]}]}).to_string()).is_err());
    store
        .save(None, "preference", "直接说。", None, None)
        .unwrap();
    assert!(store.finish(&job, r#"{"memories":[]}"#).is_err());
    assert!(store.claim(&a).unwrap().is_none());
}

#[test]
fn retention_does_not_depend_on_ai_and_main_file_has_a_hard_limit() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let a = fixture.dir.join("a");
    store.record(&a, "s", "prompt", &"鲸".repeat(2000)).unwrap();
    let bytes: i64 = store
        .db
        .query_row("SELECT length(CAST(body AS BLOB)) FROM events", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(bytes <= 2048);
    store
        .db
        .execute("UPDATE events SET at=?1", [now() - 8 * 86400])
        .unwrap();
    store.maintain(true).unwrap();
    let count: i64 = store
        .db
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    let page: i64 = store
        .db
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .unwrap();
    let limit: i64 = store
        .db
        .query_row("PRAGMA max_page_count", [], |r| r.get(0))
        .unwrap();
    assert_eq!(page * limit, 192 * 1024 * 1024);
}

#[test]
fn similar_suggestions_are_suppressed_after_restart() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let a = fixture.dir.join("a");
    let text = "下载入口要不要记住用户上次选的平台？";
    let id = store.suggestion(&a, text).unwrap().unwrap();
    store.reject(&id).unwrap();
    drop(store);
    let mut store = fixture.store();
    assert!(store.suggestion(&a, text).unwrap().is_none());
    assert!(
        store
            .suggestion(&fixture.dir.join("b"), text)
            .unwrap()
            .is_some()
    );
    let rejected: String = store
        .db
        .query_row(
            "SELECT feedback FROM suggestions WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rejected, "rejected");
}

#[test]
fn undo_restores_the_previous_correction_and_retention_keeps_explicit_preferences() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store
        .save(None, "preference", "直接说。", None, None)
        .unwrap();
    let item = store.overview(None, "").unwrap().items.remove(0);
    store
        .save(
            None,
            "preference",
            "多解释一点。",
            Some((&item.id, item.revision)),
            None,
        )
        .unwrap();
    store.undo(&item.id, item.revision + 1).unwrap();
    store
        .db
        .execute("UPDATE memories SET at=0,updated=0", [])
        .unwrap();
    store.maintain(true).unwrap();
    assert_eq!(store.overview(None, "").unwrap().items[0].body, "直接说。");
}

#[test]
fn wal_backpressure_stops_writes_until_the_reader_releases_its_snapshot() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store
        .db
        .execute_batch(
            "CREATE TABLE test_padding(value BLOB); INSERT INTO test_padding VALUES('start');",
        )
        .unwrap();
    let reader = rusqlite::Connection::open(&store.path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM test_padding;")
        .unwrap();
    for _ in 0..68 {
        store
            .db
            .execute("INSERT INTO test_padding VALUES(zeroblob(524288))", [])
            .unwrap();
    }
    assert!(store.write_ready().is_err());
    reader.execute_batch("COMMIT").unwrap();
    assert!(store.write_ready().is_ok());
    store
        .save(None, "preference", "still usable", None, None)
        .unwrap();
}

#[test]
fn sqlite_full_preserves_existing_data_and_does_not_prevent_reads() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    store
        .save(None, "preference", "keep this", None, None)
        .unwrap();
    store
        .db
        .execute_batch("CREATE TABLE test_padding(value BLOB)")
        .unwrap();
    let count: i64 = store
        .db
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    let _: i64 = store
        .db
        .query_row(&format!("PRAGMA max_page_count={}", count + 2), [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        store
            .db
            .execute("INSERT INTO test_padding VALUES(zeroblob(1048576))", [])
            .is_err()
    );
    assert_eq!(store.overview(None, "").unwrap().items[0].body, "keep this");
}

#[test]
fn event_flood_is_bounded_per_project_without_a_model() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let a = fixture.dir.join("a");
    for i in 0..950 {
        store
            .record(&a, "s", "prompt", &format!("{i} {}", "x".repeat(2040)))
            .unwrap();
    }
    let count: i64 = store
        .db
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 900);
    let pending: i64 = store
        .db
        .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(pending, 0);
}

#[test]
fn correcting_reported_memory_removes_old_evidence_and_pending_reextraction() {
    let fixture = Fixture::new();
    let mut store = fixture.store();
    let folder = fixture.dir.join("a");
    store
        .record(&folder, "s", "reported", "Agent reports SQLite is required")
        .unwrap();
    let job = store.claim(&folder).unwrap().unwrap();
    let source = &job.evidence[0].id;
    store.finish(&job, &serde_json::json!({"memories":[{"kind":"note","text":"Agent reports SQLite is required", "sources":[source]}]}).to_string()).unwrap();
    store
        .record(&folder, "s", "question", "Is this correct?")
        .unwrap();
    let item = store.overview(Some(&folder), "").unwrap().items.remove(0);
    store
        .save(
            Some(&folder),
            "note",
            "Storage is optional",
            Some((&item.id, item.revision)),
            None,
        )
        .unwrap();
    let retained: bool = store
        .db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE id=?1)",
            params![source],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!retained);
    let jobs: i64 = store
        .db
        .query_row("SELECT count(*) FROM jobs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(jobs, 0);
    assert!(store.finish(&job, r#"{"memories":[]}"#).is_err());
    store.clear().unwrap();
    drop(store);
    let mut store = fixture.store();
    assert!(store.overview(Some(&folder), "").unwrap().items.is_empty());
    assert!(
        store
            .recall(&folder, "s", "anything", 0)
            .unwrap()
            .preferences
            .is_empty()
    );
}
