CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
INSERT OR IGNORE INTO meta VALUES ('version',1),('revision',0);
CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, path TEXT NOT NULL, seen INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS events (
 id TEXT PRIMARY KEY, project TEXT NOT NULL REFERENCES projects(id), session TEXT NOT NULL,
 kind TEXT NOT NULL, body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=2048), at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS events_project ON events(project,session,at DESC);
CREATE TABLE IF NOT EXISTS memories (
 id TEXT PRIMARY KEY, project TEXT REFERENCES projects(id), kind TEXT NOT NULL,
 body TEXT NOT NULL CHECK(length(CAST(body AS BLOB))<=1024), task TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL, at INTEGER NOT NULL, updated INTEGER NOT NULL, revision INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS memories_project ON memories(project,status,kind,updated DESC);
CREATE TABLE IF NOT EXISTS versions (memory TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE, revision INTEGER NOT NULL, body TEXT NOT NULL, at INTEGER NOT NULL, PRIMARY KEY(memory,revision));
CREATE TABLE IF NOT EXISTS sources (
 memory TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE, event TEXT NOT NULL,
 kind TEXT NOT NULL, excerpt TEXT NOT NULL CHECK(length(CAST(excerpt AS BLOB))<=512), at INTEGER NOT NULL,
 PRIMARY KEY(memory,event)
);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(body, content='memories', content_rowid='rowid', tokenize='trigram');
CREATE TRIGGER IF NOT EXISTS memories_insert AFTER INSERT ON memories BEGIN
 INSERT INTO memory_search(rowid,body) VALUES(new.rowid,new.body);
END;
CREATE TRIGGER IF NOT EXISTS memories_delete AFTER DELETE ON memories BEGIN
 INSERT INTO memory_search(memory_search,rowid,body) VALUES('delete',old.rowid,old.body);
END;
CREATE TRIGGER IF NOT EXISTS memories_update AFTER UPDATE OF body ON memories BEGIN
 INSERT INTO memory_search(memory_search,rowid,body) VALUES('delete',old.rowid,old.body);
 INSERT INTO memory_search(rowid,body) VALUES(new.rowid,new.body);
END;
CREATE TABLE IF NOT EXISTS suggestions (
 id TEXT PRIMARY KEY, project TEXT NOT NULL REFERENCES projects(id), body TEXT NOT NULL,
 feedback TEXT NOT NULL DEFAULT '', at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS suggestions_project ON suggestions(project,at DESC);
CREATE TABLE IF NOT EXISTS recalls (
 id INTEGER PRIMARY KEY, memory TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
 session TEXT NOT NULL, task TEXT NOT NULL, reason TEXT NOT NULL, at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS recalls_memory ON recalls(memory,session,task,at);
CREATE TABLE IF NOT EXISTS jobs (
 project TEXT PRIMARY KEY REFERENCES projects(id), session TEXT NOT NULL, checkout TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'pending', revision INTEGER NOT NULL DEFAULT 0, at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS attempts (at INTEGER NOT NULL);
PRAGMA user_version=1;
