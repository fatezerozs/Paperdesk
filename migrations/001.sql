PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA busy_timeout=5000;
-- JSON documents retain extension fields without coupling agent providers to migrations.
-- IDs are indexed; JSON checks reject corrupt writes. All related writes use transactions.
CREATE TABLE IF NOT EXISTS papers (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));
CREATE TABLE IF NOT EXISTS questions (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));
CREATE TABLE IF NOT EXISTS quizzes (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));
CREATE TABLE IF NOT EXISTS jobs (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));
CREATE INDEX IF NOT EXISTS questions_paper ON questions(json_extract(body, '$.paper_id'));
CREATE INDEX IF NOT EXISTS questions_status ON questions(json_extract(body, '$.status'));
CREATE TABLE IF NOT EXISTS fonts (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));

CREATE TABLE IF NOT EXISTS imports (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));

CREATE TABLE IF NOT EXISTS settings (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));
CREATE TABLE IF NOT EXISTS archived_papers (id TEXT PRIMARY KEY, body TEXT NOT NULL CHECK(json_valid(body)));

CREATE TABLE IF NOT EXISTS question_region_history (id TEXT PRIMARY KEY, body TEXT NOT NULL);
