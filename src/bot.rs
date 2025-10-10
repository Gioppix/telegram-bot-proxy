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
            if !crate::db::validate_channel_name(&channel_name) {
                bot.send_message(
                    msg.chat.id,
                    "Invalid channel name. Only letters, numbers, and underscores are allowed.",
                )
                .await?;
                return Ok(());
            }

            match crate::db::subscribe(&pool, msg.chat.id.0, &channel_name).await {
                Ok(_) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Successfully subscribed to '{}'", channel_name),
                    )
                    .await?;
                }
                Err(e) => {
                    let error_msg = if e.to_string().contains("UNIQUE constraint failed") {
                        format!("You are already subscribed to '{}'", channel_name)
                    } else {
                        format!("Error subscribing to '{}': {}", channel_name, e)
                    };
                    bot.send_message(msg.chat.id, error_msg).await?;
                }
            }
        }
        Command::Unsubscribe(channel_name) => {
            if !crate::db::validate_channel_name(&channel_name) {
                bot.send_message(
                    msg.chat.id,
                    "Invalid channel name. Only letters, numbers, and underscores are allowed.",
                )
                .await?;
                return Ok(());
            }

            match crate::db::unsubscribe(&pool, msg.chat.id.0, &channel_name).await {
                Ok(true) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Successfully unsubscribed from '{}'", channel_name),
                    )
                    .await?;
                }
                Ok(false) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("You are not subscribed to '{}'", channel_name),
                    )
                    .await?;
                }
                Err(e) => {
                    bot.send_message(
                        msg.chat.id,
                        format!("Error unsubscribing from '{}': {}", channel_name, e),
                    )
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
    #[command(description = "Unsubscribe from a channel")]
    Unsubscribe(String),
}
