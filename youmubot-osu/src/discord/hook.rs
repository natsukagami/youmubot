use super::OsuClient;
use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::oppai_cache::{BeatmapCache, BeatmapInfo},
    models::{Beatmap, Mode, Mods},
    request::BeatmapRequestKind,
};
use lazy_static::lazy_static;
use regex::Regex;
use serenity::{builder::CreateMessage, model::channel::Message, utils::MessageBuilder};
use std::str::FromStr;
use youmubot_prelude::*;

use super::embeds::{beatmap_embed, beatmapset_embed};

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
        let last_beatmap = stream::select(old_links, stream::select(new_links, short_links))
            .then(|l| async move {
                let mut bm: Option<super::BeatmapWithMode> = None;
                msg.channel_id
                    .send_message(&ctx, |m| match l.embed {
                        EmbedType::Beatmap(b, info, mods) => {
                            let t = handle_beatmap(&b, info, l.link, l.mode, mods, m);
                            let mode = l.mode.unwrap_or(b.mode);
                            bm = Some(super::BeatmapWithMode(b, mode));
                            t
                        }
                        EmbedType::Beatmapset(b) => handle_beatmapset(b, l.link, l.mode, m),
                    })
                    .await?;
                let r: Result<_> = Ok(bm);
                r
            })
            .filter_map(|v| async move {
                match v {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("{}", e);
                        None
                    }
                }
            })
            .fold(None, |_, v| async move { Some(v) })
            .await;

        // Save the beatmap for query later.
        if let Some(t) = last_beatmap {
            super::cache::save_beatmap(&*ctx.data.read().await, msg.channel_id, &t)?;
        }
        Ok(())
    })
}

enum EmbedType {
    Beatmap(Beatmap, Option<BeatmapInfo>, Mods),
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
            let osu = data.get::<OsuClient>().unwrap();
            let cache = data.get::<BeatmapCache>().unwrap();
            let req_type = capture.name("link_type").unwrap().as_str();
            let req = match req_type {
                "b" => BeatmapRequestKind::Beatmap(capture["id"].parse()?),
                "s" => BeatmapRequestKind::Beatmapset(capture["id"].parse()?),
                _ => unreachable!(),
            };
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
            let beatmaps = osu
                .beatmaps(req, |v| match mode {
                    Some(m) => v.mode(m, true),
                    None => v,
                })
                .await?;
            if beatmaps.is_empty() {
                return Ok(None);
            }
            let r: Result<_> = Ok(match req_type {
                "b" => {
                    let b = beatmaps.into_iter().next().unwrap();
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .map(|v| Mods::from_str(v.as_str()).ok())
                        .flatten()
                        .unwrap_or(Mods::NOMOD);
                    let info = match mode.unwrap_or(b.mode).to_oppai_mode() {
                        Some(mode) => cache
                            .get_beatmap(b.beatmap_id)
                            .await
                            .and_then(|b| b.get_info_with(Some(mode), mods))
                            .ok(),
                        None => None,
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
            let osu = data.get::<OsuClient>().unwrap();
            let cache = data.get::<BeatmapCache>().unwrap();
            let mode = capture
                .name("mode")
                .and_then(|v| Mode::parse_from_new_site(v.as_str()));
            let link = capture.get(0).unwrap().as_str();
            let req = match capture.name("beatmap_id") {
                Some(ref v) => BeatmapRequestKind::Beatmap(v.as_str().parse()?),
                None => BeatmapRequestKind::Beatmapset(
                    capture.name("set_id").unwrap().as_str().parse()?,
                ),
            };
            let beatmaps = osu
                .beatmaps(req, |v| match mode {
                    Some(m) => v.mode(m, true),
                    None => v,
                })
                .await?;
            if beatmaps.is_empty() {
                return Ok(None);
            }
            let r: Result<_> = Ok(match capture.name("beatmap_id") {
                Some(_) => {
                    let beatmap = beatmaps.into_iter().next().unwrap();
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .and_then(|v| Mods::from_str(v.as_str()).ok())
                        .unwrap_or(Mods::NOMOD);
                    let info = match mode.unwrap_or(beatmap.mode).to_oppai_mode() {
                        Some(mode) => cache
                            .get_beatmap(beatmap.beatmap_id)
                            .await
                            .and_then(|b| b.get_info_with(Some(mode), mods))
                            .ok(),
                        None => None,
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
                .and_then(|v| Mods::from_str(v.as_str()).ok())
                .unwrap_or(Mods::NOMOD);
            let info = match mode.unwrap_or(beatmap.mode).to_oppai_mode() {
                Some(mode) => cache
                    .get_beatmap(beatmap.beatmap_id)
                    .await
                    .and_then(|b| b.get_info_with(Some(mode), mods))
                    .ok(),
                None => None,
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

fn handle_beatmap<'a, 'b>(
    beatmap: &Beatmap,
    info: Option<BeatmapInfo>,
    link: &'_ str,
    mode: Option<Mode>,
    mods: Mods,
    m: &'a mut CreateMessage<'b>,
) -> &'a mut CreateMessage<'b> {
    m.content(
        MessageBuilder::new()
            .push("Beatmap information for ")
            .push_mono_safe(link)
            .build(),
    )
    .embed(|b| beatmap_embed(beatmap, mode.unwrap_or(beatmap.mode), mods, info, b))
}

fn handle_beatmapset<'a, 'b>(
    beatmaps: Vec<Beatmap>,
    link: &'_ str,
    mode: Option<Mode>,
    m: &'a mut CreateMessage<'b>,
) -> &'a mut CreateMessage<'b> {
    let mut beatmaps = beatmaps;
    beatmaps.sort_by(|a, b| {
        (mode.unwrap_or(a.mode) as u8, a.difficulty.stars)
            .partial_cmp(&(mode.unwrap_or(b.mode) as u8, b.difficulty.stars))
            .unwrap()
    });
    m.content(
        MessageBuilder::new()
            .push("Beatmapset information for ")
            .push_mono_safe(link)
            .build(),
    )
    .embed(|b| beatmapset_embed(&beatmaps, mode, b))
}
