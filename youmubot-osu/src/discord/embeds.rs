use super::BeatmapWithMode;
use crate::{
    discord::oppai_cache::{Accuracy, BeatmapContent, BeatmapInfo, BeatmapInfoWithPP},
    models::{Beatmap, Difficulty, Mode, Mods, Rank, Score, User},
    UserHeader,
};
use serenity::{
    all::CreateAttachment,
    builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter},
    utils::MessageBuilder,
};
use std::{borrow::Cow, time::Duration};
use youmubot_prelude::*;

/// Writes a number grouped in groups of 3.
pub(crate) fn grouped_number(num: u64) -> String {
    let s = num.to_string();
    let mut b = MessageBuilder::new();
    let mut i = if s.len() % 3 == 0 { 3 } else { s.len() % 3 };
    b.push(&s[..i]);
    while i < s.len() {
        b.push(",").push(&s[i..i + 3]);
        i += 3;
    }
    b.build()
}

fn beatmap_description(b: &Beatmap) -> String {
    MessageBuilder::new()
        .push_bold_line(b.approval.to_string())
        .push({
            let link = b.download_link(crate::BeatmapSite::Bancho);
            format!(
                "Download: [[Link]]({}) [[No Video]]({}?noVideo=1) [[BeatConnect]]({}) [[Chimu]]({})",
                link,
                link,
                b.download_link(crate::BeatmapSite::Beatconnect),
                b.download_link(crate::BeatmapSite::Chimu),
            )
        })
        .push_line(format!(" [[Beatmapset]]({})", b.beatmapset_link()))
        .push("Language: ")
        .push_bold(b.language.to_string())
        .push(" | Genre: ")
        .push_bold_line(b.genre.to_string())
        .push(
            b.source
                .as_ref()
                .map(|v| format!("Source: **{}**\n", v))
                .unwrap_or_else(|| "".to_owned()),
        )
        .push("Tags: ")
        .push_line(
            b.tags
                .iter()
                .map(|v| MessageBuilder::new().push_mono_safe(v).build())
                .take(10)
                .chain(std::iter::once("...".to_owned()))
                .collect::<Vec<_>>()
                .join(" "),
        )
        .build()
}

pub fn beatmap_offline_embed(
    b: &'_ crate::discord::oppai_cache::BeatmapContent,
    m: Mode,
    mods: &Mods,
) -> Result<(CreateEmbed, Vec<CreateAttachment>)> {
    let bm = b.content.clone();
    let metadata = b.metadata.clone();
    let (info, pp) = b.get_possible_pp_with(m, mods)?;

    let total_length = if !bm.hit_objects.is_empty() {
        Duration::from_millis(
            (bm.hit_objects.last().unwrap().start_time - bm.hit_objects.first().unwrap().start_time)
                as u64,
        )
    } else {
        Duration::from_secs(0)
    };

    let (circles, sliders, spinners) = {
        let (mut circles, mut sliders, mut spinners) = (0u64, 0u64, 0u64);
        for obj in bm.hit_objects.iter() {
            match obj.kind {
                rosu_pp::model::hit_object::HitObjectKind::Circle => circles += 1,
                rosu_pp::model::hit_object::HitObjectKind::Slider(_) => sliders += 1,
                rosu_pp::model::hit_object::HitObjectKind::Spinner(_) => spinners += 1,
                rosu_pp::model::hit_object::HitObjectKind::Hold(_) => sliders += 1,
            }
        }
        (circles, sliders, spinners)
    };

    let diff = Difficulty {
        stars: info.stars,
        aim: None,   // TODO: this is currently unused
        speed: None, // TODO: this is currently unused
        cs: bm.cs as f64,
        od: bm.od as f64,
        ar: bm.ar as f64,
        hp: bm.hp as f64,
        count_normal: circles,
        count_slider: sliders,
        count_spinner: spinners,
        max_combo: Some(info.max_combo as u64),
        bpm: bm.bpm(),
        drain_length: total_length, // It's hard to calculate so maybe just skip...
        total_length,
    }
    .apply_mods(mods, info.stars);
    let mut embed = CreateEmbed::new()
        .title(beatmap_title(
            &metadata.artist,
            &metadata.title,
            &metadata.version,
            mods,
        ))
        .author({
            CreateEmbedAuthor::new(&metadata.creator)
                .url(format!("https://osu.ppy.sh/users/{}", metadata.creator))
        })
        .color(0xffb6c1)
        .field(
            "Calculated pp",
            format!(
                "95%: **{:.2}**pp, 98%: **{:.2}**pp, 99%: **{:.2}**pp, 100%: **{:.2}**pp",
                pp[0], pp[1], pp[2], pp[3]
            ),
            false,
        )
        .field("Information", diff.format_info(m, &mods, None), false);
    let mut attachments = Vec::new();
    if let Some(bg) = &b.beatmap_background {
        embed = embed.thumbnail(format!("attachment://{}", bg.filename));
        attachments.push(CreateAttachment::bytes(
            bg.content.clone().into_vec(),
            bg.filename.clone(),
        ));
    }

    Ok((embed, attachments))
}

// Some helper functions here

/// Create a properly formatted beatmap title, in the `Artist - Title [Difficulty] +mods` format.
fn beatmap_title(
    artist: impl AsRef<str>,
    title: impl AsRef<str>,
    difficulty: impl AsRef<str>,
    mods: &Mods,
) -> String {
    let mod_str = if mods == Mods::NOMOD {
        "".to_owned()
    } else {
        format!(" {}", mods)
    };
    MessageBuilder::new()
        .push_bold_safe(artist.as_ref())
        .push(" - ")
        .push_bold_safe(title.as_ref())
        .push(" [")
        .push_bold_safe(difficulty.as_ref())
        .push("]")
        .push(&mod_str)
        .build()
}

pub fn beatmap_embed(b: &'_ Beatmap, m: Mode, mods: &Mods, info: BeatmapInfoWithPP) -> CreateEmbed {
    let diff = b.difficulty.apply_mods(mods, info.0.stars);
    CreateEmbed::new()
        .title(beatmap_title(&b.artist, &b.title, &b.difficulty_name, mods))
        .author(
            CreateEmbedAuthor::new(&b.creator)
                .url(format!("https://osu.ppy.sh/users/{}", b.creator_id))
                .icon_url(format!("https://a.ppy.sh/{}", b.creator_id)),
        )
        .url(b.link())
        .image(b.cover_url())
        .color(0xffb6c1)
        .fields({
            let pp = info.1;
            std::iter::once((
                "Calculated pp",
                format!(
                    "95%: **{:.2}**pp, 98%: **{:.2}**pp, 99%: **{:.2}**pp, 100%: **{:.2}**pp",
                    pp[0], pp[1], pp[2], pp[3]
                ),
                false,
            ))
        })
        .field("Information", diff.format_info(m, &mods, b), false)
        .description(beatmap_description(b))
}

const MAX_DIFFS: usize = 25 - 4;

pub fn beatmapset_embed(bs: &'_ [Beatmap], m: Option<Mode>) -> CreateEmbed {
    let too_many_diffs = bs.len() > MAX_DIFFS;
    let b: &Beatmap = &bs[0];
    let mut m = CreateEmbed::new()
        .title(
            MessageBuilder::new()
                .push_bold_safe(&b.artist)
                .push(" - ")
                .push_bold_safe(&b.title)
                .build(),
        )
        .author(
            CreateEmbedAuthor::new(&b.creator)
                .url(format!("https://osu.ppy.sh/users/{}", b.creator_id))
                .icon_url(format!("https://a.ppy.sh/{}", b.creator_id)),
        )
        .url(format!(
            "https://osu.ppy.sh/beatmapsets/{}",
            b.beatmapset_id,
        ))
        .image(format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            b.beatmapset_id
        ))
        .color(0xffb6c1)
        .description(beatmap_description(b))
        .fields(bs.iter().rev().take(MAX_DIFFS).rev().map(|b: &Beatmap| {
            (
                format!("[{}]", b.difficulty_name),
                b.difficulty
                    .format_info(m.unwrap_or(b.mode), Mods::NOMOD, b),
                false,
            )
        }));
    if too_many_diffs {
        m = m.footer(CreateEmbedFooter::new(format!(
            "This map has {} diffs, we are showing the last {}.",
            bs.len(),
            MAX_DIFFS
        )));
    }
    m
}

pub(crate) struct ScoreEmbedBuilder<'a> {
    s: &'a Score,
    bm: &'a BeatmapWithMode,
    content: &'a BeatmapContent,
    u: UserHeader,
    top_record: Option<u8>,
    world_record: Option<u16>,
    footer: Option<String>,
}

impl<'a> ScoreEmbedBuilder<'a> {
    pub fn top_record(&mut self, rank: u8) -> &mut Self {
        self.top_record = Some(rank);
        self
    }
    pub fn world_record(&mut self, rank: u16) -> &mut Self {
        self.world_record = Some(rank);
        self
    }
    pub fn footer(&mut self, footer: impl Into<String>) -> &mut Self {
        self.footer = Some(footer.into());
        self
    }
}

pub(crate) fn score_embed<'a>(
    s: &'a Score,
    bm: &'a BeatmapWithMode,
    content: &'a BeatmapContent,
    u: impl Into<UserHeader>,
) -> ScoreEmbedBuilder<'a> {
    ScoreEmbedBuilder {
        s,
        bm,
        content,
        u: u.into(),
        top_record: None,
        world_record: None,
        footer: None,
    }
}

impl<'a> ScoreEmbedBuilder<'a> {
    #[allow(clippy::many_single_char_names)]
    pub fn build(&mut self) -> CreateEmbed {
        let mode = self.bm.mode();
        let b = &self.bm.0;
        let s = self.s;
        let content = self.content;
        let u = &self.u;
        let accuracy = s.accuracy(mode);
        let info = content.get_info_with(mode, &s.mods).ok();
        let stars = info
            .as_ref()
            .map(|info| info.stars)
            .unwrap_or(b.difficulty.stars);
        let score_line = match s.rank {
            Rank::SS | Rank::SSH => "SS".to_string(),
            _ if s.perfect => format!("{:.2}% FC", accuracy),
            Rank::F => {
                let display = info
                    .map(|info| {
                        ((s.count_300 + s.count_100 + s.count_50 + s.count_miss) as f64)
                            / (info.objects as f64)
                            * 100.0
                    })
                    .map(|p| format!("FAILED @ {:.2}%", p))
                    .unwrap_or_else(|| "FAILED".to_owned());
                format!("{:.2}% {} combo [{}]", accuracy, s.max_combo, display)
            }
            v => format!(
                "{:.2}% {}x {} miss {} rank",
                accuracy, s.max_combo, s.count_miss, v
            ),
        };
        let pp = s.pp.map(|pp| (pp, format!("{:.2}pp", pp))).or_else(|| {
            content
                .get_pp_from(
                    mode,
                    Some(s.max_combo as usize),
                    Accuracy::ByCount(s.count_300, s.count_100, s.count_50, s.count_miss),
                    &s.mods,
                )
                .ok()
                .map(|pp| (pp, format!("{:.2}pp [?]", pp)))
        });
        let pp = if !s.perfect {
            content
                .get_pp_from(
                    mode,
                    None,
                    Accuracy::ByCount(s.count_300 + s.count_miss, s.count_100, s.count_50, 0),
                    &s.mods,
                )
                .ok()
                .filter(|&v| pp.as_ref().map(|&(origin, _)| origin < v).unwrap_or(false))
                .and_then(|value| {
                    pp.as_ref()
                        .map(|(_, original)| format!("{} ({:.2}pp if FC?)", original, value))
                })
                .or_else(|| pp.map(|v| v.1))
        } else {
            pp.map(|v| v.1)
        };
        let pp_gained = {
            let effective_pp = s.effective_pp.or_else(|| {
                s.pp.zip(self.top_record)
                    .map(|(pp, top)| pp * (0.95f64).powi(top as i32 - 1))
            });
            match (s.pp, effective_pp) {
                (Some(pp), Some(epp)) => Some(format!(
                    "**pp gained**: **{:.2}**pp (**+{:.2}**pp)",
                    pp, epp
                )),
                (Some(pp), None) => Some(format!("**pp gained**: **{:.2}**pp", pp)),
                _ => None,
            }
        };
        let score_line = pp
            .map(|pp| format!("{} | {}", &score_line, pp))
            .unwrap_or(score_line);
        let max_combo = b
            .difficulty
            .max_combo
            .map(|max| format!("**{}x**/{}x", s.max_combo, max))
            .unwrap_or_else(|| format!("**{}x**", s.max_combo));
        let top_record = self
            .top_record
            .map(|v| format!(" | #{} top record!", v))
            .unwrap_or_else(|| "".to_owned());
        let world_record = self
            .world_record
            .map(|v| v as u32)
            .or(s.global_rank)
            .map(|v| format!(" | #{} on Global Rankings!", v))
            .unwrap_or_else(|| "".to_owned());
        let diff = b.difficulty.apply_mods(&s.mods, stars);
        let creator = if b.difficulty_name.contains("'s") {
            "".to_owned()
        } else {
            format!("by {} ", b.creator)
        };
        let mod_details = {
            let mut d = s.mods.details();
            if d.is_empty() {
                None
            } else {
                d.insert(0, "**Mods**:".to_owned());
                Some(Cow::from(d.join("\n- ")))
            }
        };
        let description_fields = [
            Some(
                format!(
                    "**Played**: {} {} {}",
                    s.date.format("<t:%s:R>"),
                    s.link()
                        .map(|s| format!("[[Score]]({})", s).into())
                        .unwrap_or(Cow::from("")),
                    s.replay_download_link()
                        .map(|s| format!("[[Replay]]({})", s).into())
                        .unwrap_or(Cow::from("")),
                )
                .into(),
            ),
            pp_gained.as_ref().map(|v| (&v[..]).into()),
            mod_details,
        ]
        .into_iter()
        .filter_map(|v: Option<Cow<'_, str>>| v)
        .collect::<Vec<_>>()
        .join("\n");
        let mut m = CreateEmbed::new()
            .author(
                CreateEmbedAuthor::new(&u.username)
                    .url(u.link())
                    .icon_url(u.avatar_url()),
            )
            .color(0xffb6c1)
            .title(
                MessageBuilder::new()
                    .push_safe(&u.username)
                    .push(" | ")
                    .push_safe(&b.artist)
                    .push(" - ")
                    .push(&b.title)
                    .push(" [")
                    .push_safe(&b.difficulty_name)
                    .push("] ")
                    .push(s.mods.to_string())
                    .push(" ")
                    .push(format!("({:.2}\\*)", stars))
                    .push(" ")
                    .push_safe(creator)
                    .push("| ")
                    .push(score_line)
                    .push(top_record)
                    .push(world_record)
                    .build(),
            )
            .description(description_fields)
            .thumbnail(b.thumbnail_url())
            .field(
                "Score stats",
                format!(
                    "**{}** | {} | **{:.2}%**",
                    grouped_number(s.score.unwrap_or(s.normalized_score as u64)),
                    max_combo,
                    accuracy
                ),
                true,
            )
            .field(
                "300s | 100s | 50s | misses",
                format!(
                    "**{}** ({}) | **{}** ({}) | **{}** | **{}**",
                    s.count_300, s.count_geki, s.count_100, s.count_katu, s.count_50, s.count_miss
                ),
                true,
            )
            .field("Map stats", diff.format_info(mode, &s.mods, b), false);
        let mut footer = self.footer.take().unwrap_or_default();
        if mode != Mode::Std && &s.mods != Mods::NOMOD {
            footer += " Star difficulty does not reflect game mods.";
        }
        if !footer.is_empty() {
            m = m.footer(CreateEmbedFooter::new(footer));
        }
        m
    }
}

pub(crate) fn user_embed(
    u: User,
    map_length: f64,
    map_age: i64,
    best: Option<(Score, BeatmapWithMode, BeatmapInfo)>,
) -> CreateEmbed {
    let mut stats = Vec::<(&'static str, String, bool)>::new();
    if map_length > 0.0 {
        stats.push((
            "Weighted Map Length",
            {
                let secs = map_length.floor() as u64;
                let minutes = secs / 60;
                let seconds = map_length - (60 * minutes) as f64;
                format!(
                    "**{}**mins **{:05.2}**s (**{:.2}**s)",
                    minutes, seconds, map_length
                )
            },
            true,
        ))
    }
    if map_age > 0 {
        stats.push(("Weighted Map Age", format!("<t:{}:F>", map_age), true))
    }
    CreateEmbed::new()
        .title(MessageBuilder::new().push_safe(u.username).build())
        .url(format!("https://osu.ppy.sh/users/{}", u.id))
        .color(0xffb6c1)
        .thumbnail(format!("https://a.ppy.sh/{}", u.id))
        .description(format!("Member since **{}**", u.joined.format("<t:%s:R>")))
        .field(
            "Performance Points",
            u.pp.map(|v| format!("{:.2}pp", v))
                .unwrap_or_else(|| "Inactive".to_owned()),
            false,
        )
        .field("World Rank", format!("#{}", grouped_number(u.rank)), true)
        .field(
            "Country Rank",
            format!(
                ":flag_{}: #{}",
                u.country.to_lowercase(),
                grouped_number(u.country_rank)
            ),
            true,
        )
        .field("Accuracy", format!("{:.2}%", u.accuracy), true)
        .field(
            "Play count / Play time",
            format!(
                "{} / {} hours ({})",
                grouped_number(u.play_count),
                u.played_time.as_secs() / 3600,
                Duration(u.played_time)
            ),
            false,
        )
        .field(
            "Ranks",
            format!(
                "**{}** SSH | **{}** SS | **{}** SH | **{}** S | **{}** A",
                grouped_number(u.count_ssh),
                grouped_number(u.count_ss),
                grouped_number(u.count_sh),
                grouped_number(u.count_s),
                grouped_number(u.count_a)
            ),
            false,
        )
        .fields(stats)
        .field(
            format!("Level {:.0}", u.level),
            format!(
                "**{}** total score, **{}** ranked score",
                grouped_number(u.total_score),
                grouped_number(u.ranked_score)
            ),
            false,
        )
        .fields(best.map(|(v, map, info)| {
            let BeatmapWithMode(map, mode) = map;
            (
                "Best Record",
                MessageBuilder::new()
                    .push_bold(format!(
                        "{:.2}pp",
                        v.pp.unwrap() /*Top record should have pp*/
                    ))
                    .push(" - ")
                    .push_line(v.date.format("<t:%s:R>").to_string())
                    .push("on ")
                    .push_line(format!(
                        "[{} - {} [{}]]({})**{} **",
                        MessageBuilder::new().push_bold_safe(&map.artist).build(),
                        MessageBuilder::new().push_bold_safe(&map.title).build(),
                        map.difficulty_name,
                        map.link(),
                        v.mods,
                    ))
                    .push(format!(
                        "> {}",
                        map.difficulty
                            .apply_mods(&v.mods, info.stars)
                            .format_info(mode, &v.mods, &map)
                            .replace('\n', "\n> ")
                    ))
                    .build(),
                false,
            )
        }))
}
