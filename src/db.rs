use std::str::FromStr;

use anyhow::Result;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect_lazy_with(
        SqliteConnectOptions::from_str(database_url)?.create_if_missing(true),
    );

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub fn validate_channel_name(channel_name: &str) -> bool {
    if channel_name.is_empty() {
        return false;
    }
    channel_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
}

pub async fn subscribe(pool: &SqlitePool, telegram_id: i64, channel_name: &str) -> Result<()> {
    if !validate_channel_name(channel_name) {
        return Err(anyhow::anyhow!("Invalid channel name"));
    }

    sqlx::query!(
        "INSERT INTO subscriptions (telegram_id, channel_name) VALUES (?, ?)",
        telegram_id,
        channel_name
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn unsubscribe(pool: &SqlitePool, telegram_id: i64, channel_name: &str) -> Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM subscriptions WHERE telegram_id = ? AND channel_name = ?",
        telegram_id,
        channel_name
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_subscribers(pool: &SqlitePool, channel_name: &str) -> Result<Vec<i64>> {
    let rows = sqlx::query!(
        "SELECT telegram_id FROM subscriptions WHERE channel_name = ?",
        channel_name
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.telegram_id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_subscribe(pool: SqlitePool) -> Result<()> {
        let result = subscribe(&pool, 123456, "news").await;
        assert!(result.is_ok());
        Ok(())
    }

    #[sqlx::test]
    async fn test_duplicate_subscription(pool: SqlitePool) -> Result<()> {
        subscribe(&pool, 123456, "news").await.unwrap();
        let result = subscribe(&pool, 123456, "news").await;
        assert!(result.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn test_get_subscribers(pool: SqlitePool) -> Result<()> {
        subscribe(&pool, 111, "tech").await.unwrap();
        subscribe(&pool, 222, "tech").await.unwrap();
        subscribe(&pool, 333, "news").await.unwrap();

        let subs = get_subscribers(&pool, "tech").await.unwrap();
        assert_eq!(subs.len(), 2);
        assert!(subs.contains(&111));
        assert!(subs.contains(&222));
        Ok(())
    }

    #[sqlx::test]
    async fn test_channel_name_with_space(pool: SqlitePool) -> Result<()> {
        let result = subscribe(&pool, 123, "invalid channel").await;
        assert!(result.is_err());
        Ok(())
    }

    #[sqlx::test]
    async fn test_unsubscribe(pool: SqlitePool) -> Result<()> {
        subscribe(&pool, 123, "news").await.unwrap();
        let result = unsubscribe(&pool, 123, "news").await.unwrap();
        assert!(result); // Should return true for successful unsubscribe

        let subs = get_subscribers(&pool, "news").await.unwrap();
        assert_eq!(subs.len(), 0);
        Ok(())
    }

    #[sqlx::test]
    async fn test_unsubscribe_not_subscribed(pool: SqlitePool) -> Result<()> {
        let result = unsubscribe(&pool, 123, "news").await.unwrap();
        assert!(!result); // Should return false when not subscribed
        Ok(())
    }

    #[test]
    fn test_validate_channel_name() {
        assert!(validate_channel_name("valid_channel123"));
        assert!(validate_channel_name("channel"));
        assert!(validate_channel_name("channel_123"));
        assert!(validate_channel_name("CHANNEL_123"));
        assert!(!validate_channel_name("invalid channel"));
        assert!(!validate_channel_name("invalid-channel"));
        assert!(!validate_channel_name("invalid.channel"));
        assert!(!validate_channel_name(""));
    }

    #[sqlx::test]
    async fn test_empty_channel_returns_empty(pool: SqlitePool) -> Result<()> {
        let subs = get_subscribers(&pool, "nonexistent").await.unwrap();
        assert_eq!(subs.len(), 0);
        Ok(())
    }
}
