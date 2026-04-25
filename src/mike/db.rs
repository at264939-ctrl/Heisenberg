// mike/db.rs -- PostgreSQL backing for long-term memory
// "Mike doesn't forget. He keeps ledgers."

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_postgres::{Client, NoTls};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInteraction {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub user_input: String,
    pub agent_response: Option<String>,
    pub category: Option<String>,
    pub short_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPattern {
    pub id: i32,
    pub pattern_hash: String,
    pub description: String,
    pub frequency: i32,
    pub last_seen: DateTime<Utc>,
    pub suggested_action: Option<String>,
}

pub struct DbLedger {
    client: Client,
}

impl DbLedger {
    /// Connect to PostgreSQL. Requires `DATABASE_URL` or assumes default local postgres.
    pub async fn connect() -> Result<Self> {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "host=localhost user=postgres dbname=heisenberg".to_string());
        
        let (client, connection) = tokio_postgres::connect(&db_url, NoTls)
            .await
            .context("Failed to connect to PostgreSQL. Is the database running?")?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("PostgreSQL connection error: {}", e);
            }
        });

        let mut ledger = Self { client };
        ledger.init_schema().await?;

        Ok(ledger)
    }

    /// Initialize the schema if it doesn't exist.
    async fn init_schema(&mut self) -> Result<()> {
        let schema = r#"
            CREATE TABLE IF NOT EXISTS memory_interactions (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                created_at timestamptz NOT NULL DEFAULT now(),
                user_input text NOT NULL,
                agent_response text,
                category text,
                short_summary text
            );
            CREATE INDEX IF NOT EXISTS idx_mem_inter_created ON memory_interactions(created_at);

            CREATE TABLE IF NOT EXISTS task_patterns (
                id SERIAL PRIMARY KEY,
                pattern_hash VARCHAR(64) UNIQUE NOT NULL,
                description TEXT NOT NULL,
                frequency INT DEFAULT 1,
                last_seen TIMESTAMPTZ DEFAULT NOW(),
                suggested_action TEXT
            );
        "#;
        self.client.batch_execute(schema).await?;
        Ok(())
    }

    /// Record a conversation turn
    pub async fn record_interaction(
        &self,
        user_input: &str,
        agent_response: &str,
        category: Option<&str>,
        summary: Option<&str>,
    ) -> Result<Uuid> {
        let row = self
            .client
            .query_one(
                "INSERT INTO memory_interactions (user_input, agent_response, category, short_summary)
                 VALUES ($1, $2, $3, $4) RETURNING id",
                &[&user_input, &agent_response, &category, &summary],
            )
            .await?;
        
        Ok(row.get(0))
    }

    /// Fetch recent context
    pub async fn get_recent_interactions(&self, limit: i64) -> Result<Vec<MemoryInteraction>> {
        let rows = self
            .client
            .query(
                "SELECT id, created_at, user_input, agent_response, category, short_summary 
                 FROM memory_interactions ORDER BY created_at DESC LIMIT $1",
                &[&limit],
            )
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(MemoryInteraction {
                id: row.get(0),
                created_at: row.get(1),
                user_input: row.get(2),
                agent_response: row.get(3),
                category: row.get(4),
                short_summary: row.get(5),
            });
        }
        
        // Reverse so chronological
        results.reverse();
        Ok(results)
    }

    /// Log a task pattern, incrementing frequency if it already exists.
    pub async fn observe_task(&self, description: &str, suggested_action: Option<&str>) -> Result<()> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(description.trim().to_lowercase().as_bytes());
        let hash = hex::encode(hasher.finalize());

        self.client
            .execute(
                "INSERT INTO task_patterns (pattern_hash, description, frequency, last_seen, suggested_action)
                 VALUES ($1, $2, 1, NOW(), $3)
                 ON CONFLICT (pattern_hash) DO UPDATE 
                 SET frequency = task_patterns.frequency + 1,
                     last_seen = NOW(),
                     suggested_action = COALESCE($3, task_patterns.suggested_action)",
                &[&hash, &description, &suggested_action],
            )
            .await?;
        
        Ok(())
    }

    /// Retrieve high-frequency patterns (e.g. for suggesting automation)
    pub async fn common_patterns(&self, min_freq: i32) -> Result<Vec<TaskPattern>> {
        let rows = self
            .client
            .query(
                "SELECT id, pattern_hash, description, frequency, last_seen, suggested_action 
                 FROM task_patterns WHERE frequency >= $1 ORDER BY frequency DESC LIMIT 10",
                &[&min_freq],
            )
            .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(TaskPattern {
                id: row.get(0),
                pattern_hash: row.get(1),
                description: row.get(2),
                frequency: row.get(3),
                last_seen: row.get(4),
                suggested_action: row.get(5),
            });
        }
        Ok(results)
    }
}
