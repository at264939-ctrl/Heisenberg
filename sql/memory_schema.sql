-- PostgreSQL schema for Heisenberg long-term memory
-- Stores user interactions, summaries, categories and compaction metadata.

CREATE TABLE IF NOT EXISTS memory_interactions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at timestamptz NOT NULL DEFAULT now(),
  user_input text NOT NULL,
  agent_response text,
  outcome jsonb,
  category text,
  context_ref text,
  short_summary text,
  embedding_ref text,
  metadata jsonb
);

CREATE INDEX IF NOT EXISTS idx_memory_interactions_created_at ON memory_interactions (created_at);
CREATE INDEX IF NOT EXISTS idx_memory_interactions_category ON memory_interactions (category);

-- Compact summaries to keep memory small
CREATE TABLE IF NOT EXISTS memory_compaction (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  last_compacted timestamptz NOT NULL DEFAULT now(),
  items_compacted integer NOT NULL DEFAULT 0,
  summary text,
  metadata jsonb
);

-- Optional store for embeddings (store references to on-disk blobs)
CREATE TABLE IF NOT EXISTS memory_embeddings (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  created_at timestamptz NOT NULL DEFAULT now(),
  source_interaction UUID REFERENCES memory_interactions(id) ON DELETE CASCADE,
  embedding_ref text NOT NULL,
  dims integer,
  metadata jsonb
);

-- Lightweight key-value for agent state
CREATE TABLE IF NOT EXISTS agent_kv (
  k text PRIMARY KEY,
  v jsonb,
  updated_at timestamptz NOT NULL DEFAULT now()
);
