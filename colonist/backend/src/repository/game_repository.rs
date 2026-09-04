use sqlx::{postgres::PgPool, Row};
use serde_json::Value;
use crate::game::entities::game_instance::GameInstance;

#[derive(Clone)]
pub struct GameRepository {
    pool: PgPool,
}

impl GameRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Creates any tables that do not exist yet. Each statement is issued
    /// separately: Postgres refuses multiple commands in one prepared statement.
    pub async fn migrate(&self) -> Result<(), Box<dyn std::error::Error>> {
        const STATEMENTS: [&str; 2] = [
            r#"
            CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                state_json JSONB NOT NULL,
                updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS player_sessions (
                token TEXT PRIMARY KEY,
                player_id TEXT NOT NULL,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ];

        for statement in STATEMENTS {
            sqlx::query(statement).execute(&self.pool).await?;
        }

        Ok(())
    }

    pub async fn save_game_instance(&self, instance: &GameInstance) -> Result<(), Box<dyn std::error::Error>> {
        let status = if instance.turn_manager.game_over() { "FINISHED" } else { "ACTIVE" };

        let state_json = serde_json::to_value(instance)?;

        sqlx::query(
            r#"
            INSERT INTO games (id, status, state_json)
            VALUES ($1, $2, $3)
            ON CONFLICT (id) DO UPDATE SET
                state_json = EXCLUDED.state_json,
                status = EXCLUDED.status,
                updated_at = CURRENT_TIMESTAMP
            "#
        )
            .bind(&instance.id)
            .bind(status)
            .bind(state_json)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn load_all_active(&self) -> Result<Vec<GameInstance>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT state_json FROM games WHERE status != 'FINISHED'")
            .fetch_all(&self.pool)
            .await?;

        let mut instances = Vec::new();
        for row in rows {
            let json_value: Value = row.get("state_json");

            let inst: GameInstance = serde_json::from_value(json_value)?;
            instances.push(inst);
        }
        Ok(instances)
    }
    pub async fn save_session(&self, token: uuid::Uuid, player_id: uuid::Uuid) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query(
            r#"
            INSERT INTO player_sessions (token, player_id)
            VALUES ($1, $2)
            ON CONFLICT (token) DO NOTHING
            "#
        )
            .bind(token.to_string())
            .bind(player_id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn load_all_sessions(&self) -> Result<Vec<(uuid::Uuid, uuid::Uuid)>, Box<dyn std::error::Error>> {
        let rows = sqlx::query("SELECT token, player_id FROM player_sessions")
            .fetch_all(&self.pool)
            .await?;

        let mut out = Vec::new();
        for row in rows {
            let token: String = row.get("token");
            let player_id: String = row.get("player_id");

            match (token.parse(), player_id.parse()) {
                (Ok(token), Ok(player_id)) => out.push((token, player_id)),
                _ => log::warn!("Skipping malformed session row"),
            }
        }
        Ok(out)
    }

    pub async fn delete_game_instance(&self, game_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        sqlx::query("DELETE FROM games WHERE id = $1")
            .bind(game_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}