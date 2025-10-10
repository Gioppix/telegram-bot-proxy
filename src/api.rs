use actix_web::{HttpResponse, Result, get, post, web};
use chrono::{DateTime, Utc};
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

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastResponse {
    sent: usize,
    errors: usize,
    total_subscribers: usize,
}

#[derive(Serialize, Deserialize)]
pub struct Subscription {
    telegram_id: i64,
    channel_name: String,
    created_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetSubscriptionsResponse {
    subscriptions: Vec<Subscription>,
    total: usize,
}

#[get("/health")]
pub async fn health_check() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
    })))
}

#[post("/send-message")]
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

pub struct Authenticated;

impl actix_web::FromRequest for Authenticated {
    type Error = actix_web::Error;
    type Future = std::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        let super_secret_key = std::env::var("SUPER_SECRET_KEY").unwrap_or_else(|_| String::new());

        if super_secret_key.is_empty() {
            log::error!("SUPER_SECRET_KEY is not set in environment");
            return std::future::ready(Err(actix_web::error::ErrorInternalServerError(
                serde_json::json!({
                    "error": "Server configuration error"
                }),
            )));
        }

        let auth_header = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok());

        match auth_header {
            Some(header) if header == format!("Bearer {}", super_secret_key) => {
                std::future::ready(Ok(Authenticated))
            }
            _ => std::future::ready(Err(actix_web::error::ErrorUnauthorized(
                serde_json::json!({
                    "error": "Invalid or missing authorization"
                }),
            ))),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct BroadcastRequest {
    message: String,
}

#[post("/broadcast")]
pub async fn broadcast(
    _auth: Authenticated,
    req: web::Json<BroadcastRequest>,
    pool: web::Data<SqlitePool>,
    bot: web::Data<Bot>,
) -> Result<HttpResponse> {
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

#[get("/subscriptions")]
pub async fn get_subscriptions(
    _auth: Authenticated,
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse> {
    // Get all subscriptions
    let subscriptions = match sqlx::query!(
        "
        SELECT telegram_id,
               channel_name,
               created_at
        FROM subscriptions
        ORDER BY channel_name, telegram_id
        "
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|r| Subscription {
                telegram_id: r.telegram_id,
                channel_name: r.channel_name,
                created_at: DateTime::from_timestamp(r.created_at, 0),
            })
            .collect::<Vec<_>>(),
        Err(e) => {
            log::error!("Database error: {}", e);
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database error occurred"
            })));
        }
    };

    let total = subscriptions.len();

    Ok(HttpResponse::Ok().json(GetSubscriptionsResponse {
        subscriptions,
        total,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    #[ignore = "manual"]
    async fn manual_test_get_subscriptions() {
        dotenv::dotenv().ok();
        let secret_key = std::env::var("SUPER_SECRET_KEY").expect("SUPER_SECRET_KEY must be set");

        let client = reqwest::Client::new();
        let response = client
            .get("https://telegram-proxy.up.railway.app/subscriptions")
            .header("Authorization", format!("Bearer {}", secret_key))
            .send()
            .await
            .unwrap();

        let parsed: GetSubscriptionsResponse = response.json().await.unwrap();

        println!("Total subscriptions: {}", parsed.total);
        for sub in &parsed.subscriptions {
            println!(
                "  telegram_id: {}, created_at: {:?}, channel: {}",
                sub.telegram_id,
                sub.created_at.unwrap(),
                sub.channel_name
            );
        }
    }

    #[tokio::test]
    #[ignore = "manual"]
    async fn manual_test_broadcast() {
        dotenv::dotenv().ok();
        let secret_key = std::env::var("SUPER_SECRET_KEY").expect("SUPER_SECRET_KEY must be set");

        let client = reqwest::Client::new();
        let response = client
            .post("https://telegram-proxy.up.railway.app/broadcast")
            .header("Authorization", format!("Bearer {}", secret_key))
            .json(&serde_json::json!({
                "message": "Test broadcast message"
            }))
            .send()
            .await
            .unwrap();

        let body: BroadcastResponse = response.json().await.unwrap();
        println!("Body: {:?}", body);
    }
}
