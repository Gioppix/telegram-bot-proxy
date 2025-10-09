use actix_web::{web, HttpResponse, Result};
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
    channel: String,
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

    if req.channel_name.contains(' ') {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Channel name must not contain spaces"
        })));
    }

    let subscribers = match crate::db::get_subscribers(&pool, &req.channel_name).await {
        Ok(subs) => subs,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database error"
            })));
        }
    };

    let sent = futures::future::join_all(subscribers.into_iter().map(|telegram_id| {
        let bot = bot.clone();
        let message = req.message.clone();
        async move { bot.send_message(ChatId(telegram_id), message).await.is_ok() }
    }))
    .await
    .into_iter()
    .filter(|&success| success)
    .count();

    Ok(HttpResponse::Ok().json(SendMessageResponse {
        sent,
        channel: req.channel_name.clone(),
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
