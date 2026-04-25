-- PostgreSQL memory schema for Heisenberg

CREATE TABLE IF NOT EXISTS memory_sessions (
    id UUID PRIMARY KEY,
    start_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    end_time TIMESTAMPTZ,
    context_summary TEXT
);

CREATE TABLE IF NOT EXISTS memories (
    id UUID PRIMARY KEY,
    session_id UUID REFERENCES memory_sessions(id) ON DELETE CASCADE,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    role VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    category VARCHAR(50),
    tokens_used INT
);

CREATE TABLE IF NOT EXISTS task_patterns (
    id SERIAL PRIMARY KEY,
    pattern_hash VARCHAR(64) UNIQUE NOT NULL,
    description TEXT NOT NULL,
    frequency INT DEFAULT 1,
    last_seen TIMESTAMPTZ DEFAULT NOW(),
    suggested_action TEXT
);

-- Compaction tracking to ensure we don't blow past RAM limits in Postgres buffers either
CREATE INDEX IF NOT EXISTS idx_memories_timestamp ON memories(timestamp);
CREATE INDEX IF NOT EXISTS idx_memories_session ON memories(session_id);
