use serenity::all::Member;
use youmubot_prelude::*;

use crate::{discord::args::ScoreDisplay, models::Mods};

#[poise::command(slash_command, subcommands("check"))]
pub async fn osu<T: AsRef<crate::Env> + Sync>(_ctx: CmdContext<'_, T>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command)]
/// Check your/someone's score on the last beatmap in the channel
async fn check<T: AsRef<crate::Env> + Sync>(
    ctx: CmdContext<'_, T>,
    #[description = "Pass an osu! username to check for scores"] osu_id: Option<String>,
    #[description = "Pass a member of the guild to check for scores"] member: Option<Member>,
    #[description = "Filter mods that should appear in the scores returned"] mods: Option<Mods>,
    #[description = "Score display style"] style: Option<ScoreDisplay>,
) -> Result<()> {
    todo!()
}
