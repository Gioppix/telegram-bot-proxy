use anyhow::Result;
use sqlx::SqlitePool;

pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    let pool = SqlitePool::connect(database_url).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn subscribe(pool: &SqlitePool, telegram_id: i64, channel_name: &str) -> Result<()> {
    sqlx::query!(
        "INSERT INTO subscriptions (telegram_id, channel_name) VALUES (?, ?)",
        telegram_id,
        channel_name
    )
    .execute(pool)
    .await?;
    Ok(())
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
    async fn test_empty_channel_returns_empty(pool: SqlitePool) -> Result<()> {
        let subs = get_subscribers(&pool, "nonexistent").await.unwrap();
        assert_eq!(subs.len(), 0);
        Ok(())
    }
}
