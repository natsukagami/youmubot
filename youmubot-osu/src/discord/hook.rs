use super::OsuClient;
use crate::{
    discord::oppai_cache::{BeatmapCache, BeatmapInfo},
    models::{Beatmap, Mode, Mods},
    request::BeatmapRequestKind,
};
use lazy_static::lazy_static;
use regex::Regex;
use serenity::{
    builder::CreateMessage,
    framework::standard::{CommandError as Error, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use std::str::FromStr;
use youmubot_prelude::*;

use super::embeds::{beatmap_embed, beatmapset_embed};

lazy_static! {
    static ref OLD_LINK_REGEX: Regex = Regex::new(
        r"https?://osu\.ppy\.sh/(?P<link_type>s|b)/(?P<id>\d+)(?:[\&\?]m=(?P<mode>\d))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
    static ref NEW_LINK_REGEX: Regex = Regex::new(
        r"https?://osu\.ppy\.sh/beatmapsets/(?P<set_id>\d+)/?(?:\#(?P<mode>osu|taiko|fruits|mania)(?:/(?P<beatmap_id>\d+)|/?))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
}

pub fn hook(ctx: &mut Context, msg: &Message) -> () {
    if msg.author.bot {
        return;
    }
    let mut v = move || -> CommandResult {
        let old_links = handle_old_links(ctx, &msg.content)?;
        let new_links = handle_new_links(ctx, &msg.content)?;
        let mut last_beatmap = None;
        for l in old_links.into_iter().chain(new_links.into_iter()) {
            if let Err(v) = msg.channel_id.send_message(&ctx, |m| match l.embed {
                EmbedType::Beatmap(b, info, mods) => {
                    let t = handle_beatmap(&b, info, l.link, l.mode, mods, m);
                    let mode = l.mode.unwrap_or(b.mode);
                    last_beatmap = Some(super::BeatmapWithMode(b, mode));
                    t
                }
                EmbedType::Beatmapset(b) => handle_beatmapset(b, l.link, l.mode, m),
            }) {
                println!("Error in osu! hook: {:?}", v)
            }
        }
        // Save the beatmap for query later.
        if let Some(t) = last_beatmap {
            if let Err(v) = super::cache::save_beatmap(&*ctx.data.read(), msg.channel_id, &t) {
                dbg!(v);
            }
        }
        Ok(())
    };
    if let Err(v) = v() {
        println!("Error in osu! hook: {:?}", v)
    }
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

fn handle_old_links<'a>(ctx: &mut Context, content: &'a str) -> Result<Vec<ToPrint<'a>>, Error> {
    let osu = ctx.data.get_cloned::<OsuClient>();
    let mut to_prints: Vec<ToPrint<'a>> = Vec::new();
    let cache = ctx.data.get_cloned::<BeatmapCache>();
    for capture in OLD_LINK_REGEX.captures_iter(content) {
        let req_type = capture.name("link_type").unwrap().as_str();
        let req = match req_type {
            "b" => BeatmapRequestKind::Beatmap(capture["id"].parse()?),
            "s" => BeatmapRequestKind::Beatmapset(capture["id"].parse()?),
            _ => continue,
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
        let beatmaps = osu.beatmaps(req, |v| match mode {
            Some(m) => v.mode(m, true),
            None => v,
        })?;
        match req_type {
            "b" => {
                for b in beatmaps.into_iter() {
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .map(|v| Mods::from_str(v.as_str()).ok())
                        .flatten()
                        .unwrap_or(Mods::NOMOD);
                    let info = mode.unwrap_or(b.mode).to_oppai_mode().and_then(|mode| {
                        cache
                            .get_beatmap(b.beatmap_id)
                            .and_then(|b| b.get_info_with(Some(mode), mods))
                            .ok()
                    });
                    to_prints.push(ToPrint {
                        embed: EmbedType::Beatmap(b, info, mods),
                        link: capture.get(0).unwrap().as_str(),
                        mode,
                    })
                }
            }
            "s" => to_prints.push(ToPrint {
                embed: EmbedType::Beatmapset(beatmaps),
                link: capture.get(0).unwrap().as_str(),
                mode,
            }),
            _ => (),
        }
    }
    Ok(to_prints)
}

fn handle_new_links<'a>(ctx: &mut Context, content: &'a str) -> Result<Vec<ToPrint<'a>>, Error> {
    let osu = ctx.data.get_cloned::<OsuClient>();
    let mut to_prints: Vec<ToPrint<'a>> = Vec::new();
    let cache = ctx.data.get_cloned::<BeatmapCache>();
    for capture in NEW_LINK_REGEX.captures_iter(content) {
        let mode = capture.name("mode").and_then(|v| {
            Some(match v.as_str() {
                "osu" => Mode::Std,
                "taiko" => Mode::Taiko,
                "fruits" => Mode::Catch,
                "mania" => Mode::Mania,
                _ => return None,
            })
        });
        let link = capture.get(0).unwrap().as_str();
        let req = match capture.name("beatmap_id") {
            Some(ref v) => BeatmapRequestKind::Beatmap(v.as_str().parse()?),
            None => {
                BeatmapRequestKind::Beatmapset(capture.name("set_id").unwrap().as_str().parse()?)
            }
        };
        let beatmaps = osu.beatmaps(req, |v| match mode {
            Some(m) => v.mode(m, true),
            None => v,
        })?;
        match capture.name("beatmap_id") {
            Some(_) => {
                for beatmap in beatmaps.into_iter() {
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .map(|v| Mods::from_str(v.as_str()).ok())
                        .flatten()
                        .unwrap_or(Mods::NOMOD);
                    let info = mode
                        .unwrap_or(beatmap.mode)
                        .to_oppai_mode()
                        .and_then(|mode| {
                            cache
                                .get_beatmap(beatmap.beatmap_id)
                                .and_then(|b| b.get_info_with(Some(mode), mods))
                                .ok()
                        });
                    to_prints.push(ToPrint {
                        embed: EmbedType::Beatmap(beatmap, info, mods),
                        link,
                        mode,
                    })
                }
            }
            None => to_prints.push(ToPrint {
                embed: EmbedType::Beatmapset(beatmaps),
                link,
                mode,
            }),
        }
    }
    Ok(to_prints)
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
