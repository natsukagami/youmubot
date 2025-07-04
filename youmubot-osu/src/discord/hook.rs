use std::sync::Arc;

use futures_util::stream::FuturesOrdered;
use pagination::paginate_from_fn;
use serenity::{builder::CreateMessage, model::channel::Message, utils::MessageBuilder};

use stream::Stream;
use youmubot_prelude::*;

use crate::discord::embeds::score_embed;
use crate::discord::{BeatmapWithMode, OsuEnv};
use crate::{
    discord::oppai_cache::BeatmapInfoWithPP,
    models::{Beatmap, Mode, Mods},
};

use super::embeds::beatmap_embed;
use super::interaction::{beatmap_components, score_components};
use super::link_parser::*;

/// React to /scores/{id} links.
pub fn score_hook<'a>(
    ctx: &'a Context,
    msg: &'a Message,
) -> std::pin::Pin<Box<dyn future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if msg.author.bot {
            return Ok(());
        }

        let env = {
            let data = ctx.data.read().await;
            data.get::<OsuEnv>().unwrap().clone()
        };

        let scores = parse_score_links(&msg.content)
            .into_iter()
            .map(|id| env.client.score(id))
            .collect::<FuturesOrdered<_>>()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|score| score.pls_ok().flatten());

        let embeds = scores
            .map(|score| async move {
                let env = {
                    let data = ctx.data.read().await;
                    data.get::<OsuEnv>().unwrap().clone()
                };
                let bm = env
                    .beatmaps
                    .get_beatmap(score.beatmap_id, score.mode)
                    .await?;
                let mode = score.mode;
                let content = env.oppai.get_beatmap(score.beatmap_id).await?;
                let header = env.client.user_header(score.user_id).await?.unwrap();
                Ok((score, BeatmapWithMode(bm, Some(mode)), content, header))
            })
            .collect::<FuturesOrdered<_>>()
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .filter_map(|v: Result<_>| v.pls_ok())
            .collect::<Vec<_>>();

        let len = embeds.len();
        for (i, (s, b, c, h)) in embeds.into_iter().enumerate() {
            msg.channel_id
                .send_message(
                    &ctx,
                    CreateMessage::new()
                        .reference_message(msg)
                        .content(if len == 1 {
                            "Here is the score mentioned in the message!".into()
                        } else {
                            format!(
                                "Here is the score mentioned in the message! (**{}/{}**)",
                                i + 1,
                                len
                            )
                        })
                        .embed(score_embed(&s, &b, &c, h).build())
                        .components(vec![score_components(msg.guild_id)]),
                )
                .await
                .pls_ok();
            env.last_beatmaps
                .save(msg.channel_id, &b.0, b.1)
                .await
                .pls_ok();
        }

        Ok(())
    })
}

/// React to .osz and .osu uploads.
pub fn dot_osu_hook<'a>(
    ctx: &'a Context,
    msg: &'a Message,
) -> std::pin::Pin<Box<dyn future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if msg.author.bot {
            return Ok(());
        }

        // Take all the .osu attachments
        let mut osu_embeds = msg
            .attachments
            .iter()
            .filter(
                |a| a.filename.ends_with(".osu") && a.size < 1024 * 1024, /* 1mb */
            )
            .map(|attachment| {
                let url = attachment.url.clone();

                async move {
                    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

                    let (beatmap, _) = env.oppai.download_beatmap_from_url(&url).await.ok()?;
                    let m = Mode::from(beatmap.content.mode as u8);
                    crate::discord::embeds::beatmap_offline_embed(
                        &beatmap,
                        m, /*For now*/
                        &Mods::from_str(msg.content.trim(), m, false).unwrap_or_default(),
                    )
                    .pls_ok()
                }
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(future::ready)
            .collect::<Vec<_>>()
            .await;

        const ARCHIVE_EXTS: [&str; 2] = [".osz", ".olz"];
        let osz_embeds = msg
            .attachments
            .iter()
            .filter(|a| {
                ARCHIVE_EXTS.iter().any(|ext| a.filename.ends_with(*ext))
                    && a.size < 20 * 1024 * 1024 /* 20mb */
            })
            .map(|attachment| {
                let url = attachment.url.clone();
                async move {
                    let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

                    let beatmaps = env.oppai.download_osz_from_url(&url).await.pls_ok()?;
                    Some(
                        beatmaps
                            .into_iter()
                            .filter_map(|beatmap| {
                                let m = Mode::from(beatmap.content.mode as u8);
                                crate::discord::embeds::beatmap_offline_embed(
                                    &beatmap,
                                    m, /*For now*/
                                    &Mods::from_str(msg.content.trim(), m, false)
                                        .unwrap_or_default(),
                                )
                                .pls_ok()
                            })
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(future::ready)
            .filter(|v| future::ready(!v.is_empty()))
            .collect::<Vec<_>>()
            .await
            .concat();
        osu_embeds.extend(osz_embeds);

        if !osu_embeds.is_empty() {
            let embed_len = osu_embeds.len();
            if embed_len == 1 {
                let (embed, attachments) = osu_embeds.into_iter().next().unwrap();
                msg.channel_id
                    .send_message(
                        ctx,
                        CreateMessage::new()
                            .reference_message(msg)
                            .embed(embed)
                            .add_files(attachments)
                            .content("Attached beatmap".to_owned()),
                    )
                    .await
                    .pls_ok();
            } else {
                let osu_embeds = Arc::new(osu_embeds);
                paginate_reply(
                    paginate_from_fn(|page, btns| {
                        let osu_embeds = osu_embeds.clone();
                        Box::pin(async move {
                            let (embed, attachments) = &osu_embeds[page as usize];
                            let mut edit = CreateReply::default()
                                .content(format!("Attached beatmaps ({}/{})", page + 1, embed_len))
                                .embed(embed.clone())
                                .components(btns);
                            for att in attachments {
                                edit = edit.attachment(att.clone());
                            }
                            Ok(Some(edit))
                        })
                    })
                    .with_page_count(embed_len),
                    ctx,
                    msg,
                    std::time::Duration::from_secs(180),
                )
                .await
                .pls_ok();
            }
        }

        Ok(())
    })
}

pub fn hook<'a>(
    ctx: &'a Context,
    msg: &'a Message,
) -> std::pin::Pin<Box<dyn future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if msg.author.bot {
            return Ok(());
        }
        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
        let (old_links, new_links) = (
            parse_old_links(&env, &msg.content),
            parse_new_links(&env, &msg.content),
        );
        let to_join: Box<dyn Stream<Item = _> + Unpin + Send> = {
            let use_short_link = if let Some(guild_id) = msg.guild_id {
                announcer::announcer_of(ctx, crate::discord::announcer::ANNOUNCER_KEY, guild_id)
                    .await?
                    == Some(msg.channel_id)
            } else {
                false
            };
            if use_short_link {
                Box::new(stream::select(
                    old_links,
                    stream::select(new_links, parse_short_links(&env, &msg.content)),
                ))
            } else {
                Box::new(stream::select(old_links, new_links))
            }
        };
        to_join
            .then(|l| async move {
                match l.embed {
                    EmbedType::Beatmap(b, mode, info, mods) => {
                        handle_beatmap(ctx, &b, *info, l.link, mode, mods, msg)
                            .await
                            .pls_ok();
                        let bm = super::BeatmapWithMode(*b, mode);

                        let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

                        crate::discord::cache::save_beatmap(&env, msg.channel_id, &bm)
                            .await
                            .pls_ok();
                    }
                    EmbedType::Beatmapset(b, mode) => {
                        handle_beatmapset(ctx, b, l.link, mode, msg).await.pls_ok();
                    }
                }
            })
            .collect::<()>()
            .await;

        Ok(())
    })
}

async fn handle_beatmap(
    ctx: &Context,
    beatmap: &Beatmap,
    info: BeatmapInfoWithPP,
    link: &'_ str,
    mode: Option<Mode>,
    mods: Mods,
    reply_to: &Message,
) -> Result<()> {
    reply_to
        .channel_id
        .send_message(
            ctx,
            CreateMessage::new()
                .content(
                    MessageBuilder::new()
                        .push("Beatmap information for ")
                        .push_mono_safe(link)
                        .push(" (")
                        .push(beatmap.mention(mode, &mods))
                        .push(")")
                        .build(),
                )
                .embed(beatmap_embed(
                    beatmap,
                    mode.unwrap_or(beatmap.mode),
                    &mods,
                    &info,
                ))
                .components(vec![beatmap_components(
                    mode.unwrap_or(beatmap.mode),
                    reply_to.guild_id,
                )])
                .reference_message(reply_to),
        )
        .await?;
    Ok(())
}

async fn handle_beatmapset(
    ctx: &Context,
    beatmaps: Vec<Beatmap>,
    link: &'_ str,
    mode: Option<Mode>,
    reply_to: &Message,
) -> Result<()> {
    let reply = reply_to
        .reply(
            ctx,
            format!(
                "Beatmapset information for `{}` ({})",
                link,
                beatmaps[0].beatmapset_mention()
            ),
        )
        .await?;
    crate::discord::display::display_beatmapset(
        ctx,
        beatmaps,
        mode,
        None,
        reply_to.guild_id,
        (reply, ctx),
    )
    .await
    .pls_ok();
    Ok(())
}
