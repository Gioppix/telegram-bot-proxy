mod api;
mod bot;
mod db;

use actix_web::{App, HttpServer, web};
use anyhow::Result;
use teloxide::Bot;

#[actix_web::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL").expect("DB url should be present");
    let pool = db::create_pool(&database_url).await?;
    let bot = Bot::from_env();

    let bot_pool = pool.clone();
    tokio::spawn(async move {
        // This is the poll loop, it'll never stop (hopefully)
        if let Err(e) = bot::run_bot(bot_pool).await {
            log::error!("Bot error: {}", e);
            std::process::exit(1);
        }
    });

    // Start web server
    let port = std::env::var("PORT").unwrap_or_else(|_| "8100".to_string());
    let bind_address = format!("0.0.0.0:{}", port);
    log::info!("Starting web server on {}", bind_address);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(bot.clone()))
            .service(api::health_check)
            .service(api::send_message)
            .service(api::broadcast)
            .service(api::get_subscriptions)
    })
    .bind(&bind_address)?
    .run()
    .await?;

    Ok(())
}
