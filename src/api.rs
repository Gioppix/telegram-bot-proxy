use actix_web::{HttpResponse, Result, web};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use teloxide::prelude::*;

#[derive(Deserialize, Serialize)]
pub struct SendMessageRequest {
    channel_name: String,
    message: String,
}

#[derive(Serialize)]
pub struct SendMessageResponse {
    sent: usize,
    errors: usize,
    channel: String,
}

#[derive(Deserialize)]
pub struct BroadcastRequest {
    key: String,
    message: String,
}

#[derive(Serialize)]
pub struct BroadcastResponse {
    sent: usize,
    errors: usize,
    total_subscribers: usize,
}

pub async fn send_message(
    req: web::Json<SendMessageRequest>,
    pool: web::Data<SqlitePool>,
    bot: web::Data<Bot>,
) -> Result<HttpResponse> {
    if req.message.len() > 1000 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Message too long (max 1000 chars)"
        })));
    }

    if !crate::db::validate_channel_name(&req.channel_name) {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid channel name. Only letters, numbers, and underscores are allowed."
        })));
    }

    let subscribers = match crate::db::get_subscribers(&pool, &req.channel_name).await {
        Ok(subs) => {
            if subs.is_empty() {
                return Ok(HttpResponse::Ok().json(SendMessageResponse {
                    sent: 0,
                    errors: 0,
                    channel: req.channel_name.clone(),
                }));
            }
            subs
        }
        Err(e) => {
            log::error!("Database error: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database error occurred"
            })));
        }
    };

    let results = futures::future::join_all(subscribers.into_iter().map(|telegram_id| {
        let bot = bot.clone();
        let message = req.message.clone();
        async move { bot.send_message(ChatId(telegram_id), message).await.is_ok() }
    }))
    .await;

    let sent = results.iter().filter(|&&success| success).count();
    let errors = results.len() - sent;

    Ok(HttpResponse::Ok().json(SendMessageResponse {
        sent,
        errors,
        channel: req.channel_name.clone(),
    }))
}

pub async fn broadcast(
    req: web::Json<BroadcastRequest>,
    pool: web::Data<SqlitePool>,
    bot: web::Data<Bot>,
) -> Result<HttpResponse> {
    // Validate secret key
    let super_secret_key = std::env::var("SUPER_SECRET_KEY").unwrap_or_else(|_| String::new());

    if super_secret_key.is_empty() {
        log::error!("SUPER_SECRET_KEY is not set in environment");
        return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
            "error": "Server configuration error"
        })));
    }

    if req.key != super_secret_key {
        return Ok(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid key"
        })));
    }

    // Validate message length
    if req.message.is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Message cannot be empty"
        })));
    }

    if req.message.len() > 1000 {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Message too long (max 1000 chars)"
        })));
    }

    // Get all subscribers from all channels
    let all_subscribers = match sqlx::query!(
        "
        SELECT DISTINCT telegram_id
        FROM subscriptions
        "
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(rows) => rows.into_iter().map(|r| r.telegram_id).collect::<Vec<_>>(),
        Err(e) => {
            log::error!("Database error: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database error occurred"
            })));
        }
    };

    let total_subscribers = all_subscribers.len();

    if total_subscribers == 0 {
        return Ok(HttpResponse::Ok().json(BroadcastResponse {
            sent: 0,
            errors: 0,
            total_subscribers: 0,
        }));
    }

    // Send message to all subscribers
    let results = futures::future::join_all(all_subscribers.into_iter().map(|telegram_id| {
        let bot = bot.clone();
        let message = req.message.clone();
        async move { bot.send_message(ChatId(telegram_id), message).await.is_ok() }
    }))
    .await;

    let sent = results.iter().filter(|&&success| success).count();
    let errors = results.len() - sent;

    Ok(HttpResponse::Ok().json(BroadcastResponse {
        sent,
        errors,
        total_subscribers,
    }))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore = "manual"]
    async fn manual_test_send_message() {
        let client = reqwest::Client::new();
        let response = client
            .post("http://127.0.0.1:8080/send-message")
            .json(&serde_json::json!({
                "channel_name": "ciao",
                "message": "Test message"
            }))
            .send()
            .await;

        println!("Response: {:?}", response);
    }
}
