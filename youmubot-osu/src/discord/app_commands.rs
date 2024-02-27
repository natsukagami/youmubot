use serenity::{all::Member, framework::standard::CommandResult};
use youmubot_prelude::*;

use crate::{discord::args::ScoreDisplay, models::Mods};

#[poise::command(slash_command, subcommands("check"))]
pub async fn osu<T: AsRef<crate::discord::Env> + Sync>(_ctx: CmdContext<'_, T>) -> CommandResult {
    Ok(())
}

#[poise::command(slash_command)]
/// Check your/someone's score on the last beatmap in the channel
async fn check<T: AsRef<crate::discord::Env> + Sync>(
    ctx: CmdContext<'_, T>,
    #[description = "Pass an osu! username to check for scores"] osu_id: Option<String>,
    #[description = "Pass a member of the guild to check for scores"] member: Option<Member>,
    #[description = "Filter mods that should appear in the scores returned"] mods: Option<Mods>,
    #[description = "Score display style"] style: Option<ScoreDisplay>,
) -> CommandResult {
    Ok(crate::discord::check_impl(
        ctx.data().as_ref(),
        ctx.serenity_context(),
        ctx.clone(),
        ctx.channel_id(),
        ctx.author(),
        None,
        osu_id,
        member,
        mods,
        style,
    )
    .await?)
}
