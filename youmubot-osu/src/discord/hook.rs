use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::oppai_cache::{BeatmapCache, BeatmapInfoWithPP},
    models::{Beatmap, Mode, Mods},
};
use lazy_static::lazy_static;
use regex::Regex;
use serenity::{model::channel::Message, utils::MessageBuilder};
use std::str::FromStr;
use youmubot_prelude::*;

use super::embeds::beatmap_embed;

lazy_static! {
    static ref OLD_LINK_REGEX: Regex = Regex::new(
        r"(?:https?://)?osu\.ppy\.sh/(?P<link_type>s|b)/(?P<id>\d+)(?:[\&\?]m=(?P<mode>\d))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
    static ref NEW_LINK_REGEX: Regex = Regex::new(
        r"(?:https?://)?osu\.ppy\.sh/beatmapsets/(?P<set_id>\d+)/?(?:\#(?P<mode>osu|taiko|fruits|mania)(?:/(?P<beatmap_id>\d+)|/?))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
    static ref SHORT_LINK_REGEX: Regex = Regex::new(
        r"(?:^|\s|\W)(?P<main>/b/(?P<id>\d+)(?:/(?P<mode>osu|taiko|fruits|mania))?(?:\+(?P<mods>[A-Z]+))?)"
    ).unwrap();
}

pub fn hook<'a>(
    ctx: &'a Context,
    msg: &'a Message,
) -> std::pin::Pin<Box<dyn future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        if msg.author.bot {
            return Ok(());
        }
        let (old_links, new_links, short_links) = (
            handle_old_links(ctx, &msg.content),
            handle_new_links(ctx, &msg.content),
            handle_short_links(ctx, &msg, &msg.content),
        );
        stream::select(old_links, stream::select(new_links, short_links))
            .then(|l| async move {
                match l.embed {
                    EmbedType::Beatmap(b, info, mods) => {
                        handle_beatmap(ctx, &b, info, l.link, l.mode, mods, msg)
                            .await
                            .pls_ok();
                        let mode = l.mode.unwrap_or(b.mode);
                        let bm = super::BeatmapWithMode(b, mode);
                        crate::discord::cache::save_beatmap(
                            &*ctx.data.read().await,
                            msg.channel_id,
                            &bm,
                        )
                        .await
                        .pls_ok();
                    }
                    EmbedType::Beatmapset(b) => {
                        handle_beatmapset(ctx, b, l.link, l.mode, msg)
                            .await
                            .pls_ok();
                    }
                }
            })
            .collect::<()>()
            .await;

        Ok(())
    })
}

enum EmbedType {
    Beatmap(Beatmap, Option<BeatmapInfoWithPP>, Mods),
    Beatmapset(Vec<Beatmap>),
}

struct ToPrint<'a> {
    embed: EmbedType,
    link: &'a str,
    mode: Option<Mode>,
}

fn handle_old_links<'a>(
    ctx: &'a Context,
    content: &'a str,
) -> impl stream::Stream<Item = ToPrint<'a>> + 'a {
    OLD_LINK_REGEX
        .captures_iter(content)
        .map(move |capture| async move {
            let data = ctx.data.read().await;
            let cache = data.get::<BeatmapCache>().unwrap();
            let osu = data.get::<BeatmapMetaCache>().unwrap();
            let req_type = capture.name("link_type").unwrap().as_str();
            let mode = capture
                .name("mode")
                .map(|v| v.as_str().parse())
                .transpose()?
                .and_then(|v| {
                    Some(match v {
                        0 => Mode::Std,
                        1 => Mode::Taiko,
                        2 => Mode::Catch,
                        3 => Mode::Mania,
                        _ => return None,
                    })
                });
            let beatmaps = match req_type {
                "b" => vec![match mode {
                    Some(mode) => osu.get_beatmap(capture["id"].parse()?, mode).await?,
                    None => osu.get_beatmap_default(capture["id"].parse()?).await?,
                }],
                "s" => osu.get_beatmapset(capture["id"].parse()?).await?,
                _ => unreachable!(),
            };
            if beatmaps.is_empty() {
                return Ok(None);
            }
            let r: Result<_> = Ok(match req_type {
                "b" => {
                    let b = beatmaps.into_iter().next().unwrap();
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .map(|v| Mods::from_str(v.as_str()).pls_ok())
                        .flatten()
                        .unwrap_or(Mods::NOMOD);
                    let info = match mode.unwrap_or(b.mode) {
                        Mode::Std => cache
                            .get_beatmap(b.beatmap_id)
                            .await
                            .and_then(|b| b.get_possible_pp_with(mods))
                            .pls_ok(),
                        _ => None,
                    };
                    Some(ToPrint {
                        embed: EmbedType::Beatmap(b, info, mods),
                        link: capture.get(0).unwrap().as_str(),
                        mode,
                    })
                }
                "s" => Some(ToPrint {
                    embed: EmbedType::Beatmapset(beatmaps),
                    link: capture.get(0).unwrap().as_str(),
                    mode,
                }),
                _ => None,
            });
            r
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| {
            future::ready(match v {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
            })
        })
}

fn handle_new_links<'a>(
    ctx: &'a Context,
    content: &'a str,
) -> impl stream::Stream<Item = ToPrint<'a>> + 'a {
    NEW_LINK_REGEX
        .captures_iter(content)
        .map(|capture| async move {
            let data = ctx.data.read().await;
            let osu = data.get::<BeatmapMetaCache>().unwrap();
            let cache = data.get::<BeatmapCache>().unwrap();
            let mode = capture
                .name("mode")
                .and_then(|v| Mode::parse_from_new_site(v.as_str()));
            let link = capture.get(0).unwrap().as_str();
            let beatmaps = match capture.name("beatmap_id") {
                Some(ref v) => vec![match mode {
                    Some(mode) => osu.get_beatmap(v.as_str().parse()?, mode).await?,
                    None => osu.get_beatmap_default(v.as_str().parse()?).await?,
                }],
                None => {
                    osu.get_beatmapset(capture.name("set_id").unwrap().as_str().parse()?)
                        .await?
                }
            };
            if beatmaps.is_empty() {
                return Ok(None);
            }
            let r: Result<_> = Ok(match capture.name("beatmap_id") {
                Some(_) => {
                    let beatmap = beatmaps.into_iter().next().unwrap();
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .and_then(|v| Mods::from_str(v.as_str()).pls_ok())
                        .unwrap_or(Mods::NOMOD);
                    let info = match mode.unwrap_or(beatmap.mode) {
                        Mode::Std => cache
                            .get_beatmap(beatmap.beatmap_id)
                            .await
                            .and_then(|b| b.get_possible_pp_with(mods))
                            .pls_ok(),
                        _ => None,
                    };
                    Some(ToPrint {
                        embed: EmbedType::Beatmap(beatmap, info, mods),
                        link,
                        mode,
                    })
                }
                None => Some(ToPrint {
                    embed: EmbedType::Beatmapset(beatmaps),
                    link,
                    mode,
                }),
            });
            r
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| {
            future::ready(match v {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
            })
        })
}

fn handle_short_links<'a>(
    ctx: &'a Context,
    msg: &'a Message,
    content: &'a str,
) -> impl stream::Stream<Item = ToPrint<'a>> + 'a {
    SHORT_LINK_REGEX
        .captures_iter(content)
        .map(|capture| async move {
            if let Some(guild_id) = msg.guild_id {
                if announcer::announcer_of(ctx, crate::discord::announcer::ANNOUNCER_KEY, guild_id)
                    .await?
                    != Some(msg.channel_id)
                {
                    // Disable if we are not in the server's announcer channel
                    return Err(Error::msg("not in server announcer channel"));
                }
            }
            let data = ctx.data.read().await;
            let osu = data.get::<BeatmapMetaCache>().unwrap();
            let cache = data.get::<BeatmapCache>().unwrap();
            let mode = capture
                .name("mode")
                .and_then(|v| Mode::parse_from_new_site(v.as_str()));
            let id: u64 = capture.name("id").unwrap().as_str().parse()?;
            let beatmap = match mode {
                Some(mode) => osu.get_beatmap(id, mode).await,
                None => osu.get_beatmap_default(id).await,
            }?;
            let mods = capture
                .name("mods")
                .and_then(|v| Mods::from_str(v.as_str()).pls_ok())
                .unwrap_or(Mods::NOMOD);
            let info = match mode.unwrap_or(beatmap.mode) {
                Mode::Std => cache
                    .get_beatmap(beatmap.beatmap_id)
                    .await
                    .and_then(|b| b.get_possible_pp_with(mods))
                    .pls_ok(),
                _ => None,
            };
            let r: Result<_> = Ok(ToPrint {
                embed: EmbedType::Beatmap(beatmap, info, mods),
                link: capture.name("main").unwrap().as_str(),
                mode,
            });
            r
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| {
            future::ready(match v {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("{}", e);
                    None
                }
            })
        })
}

async fn handle_beatmap<'a, 'b>(
    ctx: &Context,
    beatmap: &Beatmap,
    info: Option<BeatmapInfoWithPP>,
    link: &'_ str,
    mode: Option<Mode>,
    mods: Mods,
    reply_to: &Message,
) -> Result<()> {
    reply_to
        .channel_id
        .send_message(ctx, |m| {
            m.content(
                MessageBuilder::new()
                    .push("Beatmap information for ")
                    .push_mono_safe(link)
                    .build(),
            )
            .embed(|b| beatmap_embed(beatmap, mode.unwrap_or(beatmap.mode), mods, info, b))
            .reference_message(reply_to)
        })
        .await?;
    Ok(())
}

async fn handle_beatmapset<'a, 'b>(
    ctx: &Context,
    beatmaps: Vec<Beatmap>,
    link: &'_ str,
    mode: Option<Mode>,
    reply_to: &Message,
) -> Result<()> {
    crate::discord::display::display_beatmapset(
        &ctx,
        beatmaps,
        mode,
        None,
        reply_to,
        format!("Beatmapset information for `{}`", link),
    )
    .await
    .pls_ok();
    Ok(())
}
