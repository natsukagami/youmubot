use std::pin::Pin;

use future::Future;
use serenity::all::{
    ComponentInteractionDataKind, CreateActionRow, CreateButton, CreateInteractionResponseMessage,
    Interaction,
};
use youmubot_prelude::*;

use crate::Mods;

use super::{display::ScoreListStyle, OsuEnv};

pub(super) const BTN_CHECK: &'static str = "youmubot_osu_btn_check";
// pub(super) const BTN_LAST: &'static str = "youmubot_osu_btn_last";

/// Create an action row for score pages.
pub fn score_components() -> CreateActionRow {
    CreateActionRow::Buttons(vec![check_button()])
}

/// Create an action row for score pages.
pub fn beatmap_components() -> CreateActionRow {
    CreateActionRow::Buttons(vec![check_button()])
}

/// Creates a new check button.
pub fn check_button() -> CreateButton {
    CreateButton::new(BTN_CHECK)
        .label("Check your score")
        .emoji('🔎')
        .style(serenity::all::ButtonStyle::Secondary)
}

/// Implements the `check` button on scores and beatmaps.
pub fn handle_check_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match interaction.as_message_component() {
            Some(comp)
                if comp.data.custom_id == BTN_CHECK
                    && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
            {
                comp
            }
            _ => return Ok(()),
        };
        let (msg, author) = (&*comp.message, comp.user.id);

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
        let (bm, _) = super::load_beatmap(&env, msg).await.unwrap();
        let user_id = super::to_user_id_query(None, &env, author).await?;

        let scores = super::do_check(&env, &bm, Mods::NOMOD, &user_id).await?;

        let reply = {
            comp.create_response(
                &ctx,
                serenity::all::CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().content(format!(
                        "Here are the scores by `{}` on `{}`!",
                        &user_id,
                        bm.short_link(Mods::NOMOD)
                    )),
                ),
            )
            .await?;
            comp.get_response(&ctx).await?
        };
        ScoreListStyle::Grid
            .display_scores(scores, bm.1, ctx, reply)
            .await?;

        Ok(())
    })
}
