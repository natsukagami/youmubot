use super::*;
use youmubot_prelude::*;

/// osu!-related command group.
#[poise::command(slash_command, subcommands("top"))]
pub async fn osu<U: HasOsuEnv>(_ctx: CmdContext<'_, U>) -> Result<()> {
    Ok(())
}

/// Returns top plays for a given player.
///
/// If no osu! username is given, defaults to the currently registered user.
#[poise::command(slash_command)]
async fn top<U: HasOsuEnv>(ctx: CmdContext<'_, U>, username: Option<String>) -> Result<()> {
    todo!()
}
