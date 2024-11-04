use std::pin::Pin;

use future::Future;
use serenity::all::{
    ComponentInteraction, ComponentInteractionDataKind, CreateActionRow, CreateButton,
    CreateInteractionResponse, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
    GuildId, Interaction,
};
use youmubot_prelude::*;

use crate::{Mods, UserHeader};

use super::{
    display::ScoreListStyle,
    embeds::beatmap_embed,
    server_rank::{display_rankings_table, get_leaderboard, OrderBy},
    BeatmapWithMode, OsuEnv,
};

pub(super) const BTN_CHECK: &str = "youmubot_osu_btn_check";
pub(super) const BTN_LB: &str = "youmubot_osu_btn_lb";
pub(super) const BTN_LAST: &str = "youmubot_osu_btn_last";
pub(super) const BTN_LAST_SET: &str = "youmubot_osu_btn_last_set";

/// Create an action row for score pages.
pub fn score_components(guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button(), last_button()];
    if guild_id.is_some() {
        btns.insert(1, lb_button());
    }
    CreateActionRow::Buttons(btns)
}

/// Create an action row for score pages.
pub fn beatmap_components(guild_id: Option<GuildId>) -> CreateActionRow {
    let mut btns = vec![check_button()];
    if guild_id.is_some() {
        btns.push(lb_button());
    }
    btns.push(mapset_button());
    CreateActionRow::Buttons(btns)
}

/// Creates a new check button.
pub fn check_button() -> CreateButton {
    CreateButton::new(BTN_CHECK)
        .label("Me")
        .emoji('🔎')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `check` button on scores and beatmaps.
pub fn handle_check_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_CHECK).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
        let (bm, _) = super::load_beatmap(&env, comp.channel_id, Some(msg))
            .await
            .unwrap();
        let user = match env.saved_users.by_user_id(comp.user.id).await? {
            Some(u) => u,
            None => {
                comp.create_followup(&ctx, CreateInteractionResponseFollowup::new().content("You don't have a saved account yet! Save your osu! account by `y2!osu save <your-osu-username>`.").ephemeral(true)).await?;
                return Ok(());
            }
        };
        let header = UserHeader::from(user.clone());

        let scores = super::do_check(&env, &bm, Mods::NOMOD, &header).await?;
        if scores.is_empty() {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "No plays found for [`{}`](<https://osu.ppy.sh/users/{}>) on `{}`.",
                    user.username,
                    user.id,
                    bm.short_link(Mods::NOMOD)
                )),
            )
            .await?;
            return Ok(());
        }

        let reply = comp
            .create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "Here are the scores by [`{}`](<https://osu.ppy.sh/users/{}>) on `{}`!",
                    user.username,
                    user.id,
                    bm.short_link(Mods::NOMOD)
                )),
            )
            .await?;

        let ctx = ctx.clone();
        let guild_id = comp.guild_id;
        spawn_future(async move {
            ScoreListStyle::Grid
                .display_scores(scores, bm.1, &ctx, guild_id, reply)
                .await
                .pls_ok();
        });

        Ok(())
    })
}

/// Creates a new check button.
pub fn last_button() -> CreateButton {
    CreateButton::new(BTN_LAST)
        .label("Map")
        .emoji('🎼')
        .style(serenity::all::ButtonStyle::Success)
}

pub fn mapset_button() -> CreateButton {
    CreateButton::new(BTN_LAST_SET)
        .label("Set")
        .emoji('📚')
        .style(serenity::all::ButtonStyle::Secondary)
}

/// Implements the `last` button on scores and beatmaps.
pub fn handle_last_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LAST).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        handle_last_req(ctx, comp, false).await
    })
}

/// Implements the `beatmapset` button on scores and beatmaps.
pub fn handle_last_set_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LAST_SET).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        handle_last_req(ctx, comp, true).await
    })
}

async fn handle_last_req(
    ctx: &Context,
    comp: &ComponentInteraction,
    is_beatmapset_req: bool,
) -> Result<()> {
    let msg = &*comp.message;

    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

    let (bm, mods_def) = super::load_beatmap(&env, comp.channel_id, Some(msg))
        .await
        .unwrap();
    let BeatmapWithMode(b, m) = &bm;

    let mods = mods_def.unwrap_or_default();

    if is_beatmapset_req {
        let beatmapset = env.beatmaps.get_beatmapset(bm.0.beatmapset_id).await?;
        let reply = comp
            .create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new()
                    .content(format!("Beatmapset of `{}`", bm.short_link(&mods))),
            )
            .await?;
        super::display::display_beatmapset(
            ctx.clone(),
            beatmapset,
            None,
            mods,
            comp.guild_id,
            reply,
        )
        .await?;
        return Ok(());
    } else {
        let info = env
            .oppai
            .get_beatmap(b.beatmap_id)
            .await?
            .get_possible_pp_with(*m, &mods)?;
        comp.create_followup(
            &ctx,
            serenity::all::CreateInteractionResponseFollowup::new()
                .content(format!(
                    "Information for beatmap `{}`",
                    bm.short_link(&mods)
                ))
                .embed(beatmap_embed(b, *m, &mods, info))
                .components(vec![beatmap_components(comp.guild_id)]),
        )
        .await?;
        // Save the beatmap...
        super::cache::save_beatmap(&env, msg.channel_id, &bm).await?;
    }

    Ok(())
}

/// Creates a new check button.
pub fn lb_button() -> CreateButton {
    CreateButton::new(BTN_LB)
        .label("Ranks")
        .emoji('👑')
        .style(serenity::all::ButtonStyle::Success)
}

/// Implements the `lb` button on scores and beatmaps.
pub fn handle_lb_button<'a>(
    ctx: &'a Context,
    interaction: &'a Interaction,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let comp = match expect_and_defer_button(ctx, interaction, BTN_LB).await? {
            Some(comp) => comp,
            None => return Ok(()),
        };
        let msg = &*comp.message;

        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

        let (bm, _) = super::load_beatmap(&env, comp.channel_id, Some(msg))
            .await
            .unwrap();
        let order = OrderBy::default();
        let guild = comp.guild_id.expect("Guild-only command");

        let scores = get_leaderboard(ctx, &env, &bm, order, guild).await?;

        if scores.is_empty() {
            comp.create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(
                    "No scores have been recorded for this beatmap from anyone in this server.",
                ),
            )
            .await?;
            return Ok(());
        }

        let reply = comp
            .create_followup(
                &ctx,
                CreateInteractionResponseFollowup::new().content(format!(
                    "⌛ Loading top scores on beatmap `{}`...",
                    bm.short_link(Mods::NOMOD)
                )),
            )
            .await?;
        display_rankings_table(ctx, reply, scores, &bm, order).await?;
        Ok(())
    })
}

async fn expect_and_defer_button<'a>(
    ctx: &'_ Context,
    interaction: &'a Interaction,
    expect_id: &'static str,
) -> Result<Option<&'a ComponentInteraction>> {
    let comp = match interaction.as_message_component() {
        Some(comp)
            if comp.data.custom_id == expect_id
                && matches!(comp.data.kind, ComponentInteractionDataKind::Button) =>
        {
            comp
        }
        _ => return Ok(None),
    };
    comp.create_response(
        &ctx,
        CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
    )
    .await?;
    Ok(Some(comp))
}
