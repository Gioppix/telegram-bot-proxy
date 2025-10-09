use anyhow::Result;
use sqlx::SqlitePool;
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;

pub async fn run_bot(pool: SqlitePool) -> Result<()> {
    log::info!("Starting Telegram bot");
    let bot = Bot::from_env();

    Command::repl(bot, move |bot: Bot, msg: Message, cmd: Command| {
        let pool = pool.clone();
        async move { handle_command(bot, msg, cmd, pool).await }
    })
    .await;

    Ok(())
}

async fn handle_command(
    bot: Bot,
    msg: Message,
    cmd: Command,
    pool: SqlitePool,
) -> ResponseResult<()> {
    match cmd {
        Command::Subscribe(channel_name) => {
            if channel_name.contains(' ') {
                bot.send_message(msg.chat.id, "Channel name must not contain spaces")
                    .await?;
                return Ok(());
            }

            if channel_name.is_empty() {
                bot.send_message(msg.chat.id, "Channel name cannot be empty")
                    .await?;
                return Ok(());
            }

            match crate::db::subscribe(&pool, msg.chat.id.0, &channel_name).await {
                Ok(_) => {
                    bot.send_message(msg.chat.id, format!("Subscribed to '{}'", channel_name))
                        .await?;
                }
                Err(_) => {
                    bot.send_message(msg.chat.id, "Already subscribed or error occurred")
                        .await?;
                }
            }
        }
    }
    Ok(())
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    #[command(description = "Subscribe to a channel")]
    Subscribe(String),
}
