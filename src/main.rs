mod api;
mod bot;
mod db;

use actix_web::{web, App, HttpServer};
use anyhow::Result;
use teloxide::Bot;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:bot.db".to_string());
    let pool = db::create_pool(&database_url).await?;
    let bot = Bot::from_env();

    // Start bot in background
    let bot_pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = bot::run_bot(bot_pool).await {
            log::error!("Bot error: {}", e);
        }
    });

    // Start web server
    log::info!("Starting web server on 127.0.0.1:8080");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(bot.clone()))
            .route("/send-message", web::post().to(api::send_message))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}
