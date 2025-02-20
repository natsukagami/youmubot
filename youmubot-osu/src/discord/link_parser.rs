use std::str::FromStr;

use crate::models::*;
use lazy_static::lazy_static;
use mods::UnparsedMods;
use regex::Regex;
use stream::Stream;
use youmubot_prelude::*;

use super::{oppai_cache::BeatmapInfoWithPP, OsuEnv};

pub enum EmbedType {
    Beatmap(Box<Beatmap>, BeatmapInfoWithPP, Mods),
    Beatmapset(Vec<Beatmap>),
}

pub struct ToPrint<'a> {
    pub embed: EmbedType,
    pub link: &'a str,
    pub mode: Option<Mode>,
}

lazy_static! {
    // Beatmap(set) hooks
    static ref OLD_LINK_REGEX: Regex = Regex::new(
        r"(?:https?://)?osu\.ppy\.sh/(?P<link_type>s|b|beatmaps)/(?P<id>\d+)(?:[\&\?]m=(?P<mode>[0123]))?(?:(?P<mods>v2|[[:^alpha:]]\S+\b))?"
    ).unwrap();
    static ref NEW_LINK_REGEX: Regex = Regex::new(
        r"(?:https?://)?osu\.ppy\.sh/beatmapsets/(?P<set_id>\d+)/?(?:\#(?P<mode>osu|taiko|fruits|mania)(?:/(?P<beatmap_id>\d+)|/?))?(?:(?P<mods>v2|[[:^alpha:]]\S+\b))?"
    ).unwrap();
    static ref SHORT_LINK_REGEX: Regex = Regex::new(
        r"(?:^|\s|\W)(?P<main>/(?P<link_type>b|s)/(?P<id>\d+)(?:/(?P<mode>osu|taiko|fruits|mania))?(?:(?P<mods>v2|[[:^alpha:]]\S+\b))?)"
    ).unwrap();

    // Score hook
    pub(crate) static ref SCORE_LINK_REGEX: Regex = Regex::new(
        r"(?:https?://)?osu\.ppy\.sh/scores/(?P<score_id>\d+)"
    ).unwrap();
}

pub fn parse_old_links<'a>(
    env: &'a OsuEnv,
    content: &'a str,
) -> impl Stream<Item = ToPrint<'a>> + 'a {
    OLD_LINK_REGEX
        .captures_iter(content)
        .map(move |capture| async move {
            let req_type = capture.name("link_type").unwrap().as_str();
            let mode = capture
                .name("mode")
                .map(|v| v.as_str().parse::<u8>())
                .transpose()?
                .map(Mode::from);
            let embed = match req_type {
                "b" | "beatmaps" => {
                    // collect beatmap info
                    let mods = capture
                        .name("mods")
                        .and_then(|v| UnparsedMods::from_str(v.as_str()).pls_ok())
                        .unwrap_or_default();
                    EmbedType::from_beatmap_id(env, capture["id"].parse()?, mode, mods).await
                }
                "s" => EmbedType::from_beatmapset_id(env, capture["id"].parse()?, mode).await,
                _ => unreachable!(),
            }?;
            Ok(ToPrint {
                embed,
                link: capture.get(0).unwrap().as_str(),
                mode,
            })
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v: Result<ToPrint>| future::ready(v.pls_ok()))
}

pub fn parse_new_links<'a>(
    env: &'a OsuEnv,
    content: &'a str,
) -> impl Stream<Item = ToPrint<'a>> + 'a {
    NEW_LINK_REGEX
        .captures_iter(content)
        .map(|capture| async move {
            let mode = capture
                .name("mode")
                .and_then(|v| Mode::parse_from_new_site(v.as_str()));
            let link = capture.get(0).unwrap().as_str();
            let embed = match capture
                .name("beatmap_id")
                .map(|v| v.as_str().parse::<u64>().unwrap())
            {
                Some(beatmap_id) => {
                    let mods = capture
                        .name("mods")
                        .and_then(|v| UnparsedMods::from_str(v.as_str()).pls_ok())
                        .unwrap_or_default();
                    EmbedType::from_beatmap_id(env, beatmap_id, mode, mods).await
                }
                None => {
                    EmbedType::from_beatmapset_id(
                        env,
                        capture.name("set_id").unwrap().as_str().parse()?,
                        mode,
                    )
                    .await
                }
            }?;
            Ok(ToPrint { embed, link, mode })
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v: Result<ToPrint>| future::ready(v.pls_ok()))
}

pub fn parse_short_links<'a>(
    env: &'a OsuEnv,
    content: &'a str,
) -> impl Stream<Item = ToPrint<'a>> + 'a {
    SHORT_LINK_REGEX
        .captures_iter(content)
        .map(|capture| async move {
            let mode = capture
                .name("mode")
                .and_then(|v| Mode::parse_from_new_site(v.as_str()));
            let link = capture.name("main").unwrap().as_str();
            let id: u64 = capture.name("id").unwrap().as_str().parse()?;
            let embed = match capture.name("link_type").unwrap().as_str() {
                "b" => {
                    let mods = capture
                        .name("mods")
                        .and_then(|v| UnparsedMods::from_str(v.as_str()).pls_ok())
                        .unwrap_or_default();
                    EmbedType::from_beatmap_id(env, id, mode, mods).await?
                }
                "s" => EmbedType::from_beatmapset_id(env, id, mode).await?,
                _ => unreachable!(),
            };
            Ok(ToPrint { embed, link, mode })
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v: Result<ToPrint>| future::ready(v.pls_ok()))
}

impl EmbedType {
    async fn from_beatmap_id(
        env: &OsuEnv,
        beatmap_id: u64,
        mode: Option<Mode>,
        mods: UnparsedMods,
    ) -> Result<Self> {
        let bm = match mode {
            Some(mode) => env.beatmaps.get_beatmap(beatmap_id, mode).await?,
            None => env.beatmaps.get_beatmap_default(beatmap_id).await?,
        };
        let mods = mods.to_mods(mode.unwrap_or(bm.mode))?;
        let info = {
            let mode = mode.unwrap_or(bm.mode);
            env.oppai
                .get_beatmap(bm.beatmap_id)
                .await
                .map(|b| b.get_possible_pp_with(mode, &mods))?
        };
        Ok(Self::Beatmap(Box::new(bm), info, mods))
    }

    async fn from_beatmapset_id(
        env: &OsuEnv,
        beatmapset_id: u64,
        mode: Option<Mode>,
    ) -> Result<Self> {
        Ok(Self::Beatmapset(
            env.beatmaps.get_beatmapset(beatmapset_id, mode).await?,
        ))
    }
}
