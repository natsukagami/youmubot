use super::BeatmapWithMode;
use crate::{
    discord::oppai_cache::{BeatmapContent, BeatmapInfo, BeatmapInfoWithPP, OppaiAccuracy},
    models::{Beatmap, Mode, Mods, Rank, Score, User},
};
use chrono::Utc;
use serenity::{builder::CreateEmbed, utils::MessageBuilder};
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
        .push_bold_line(&b.approval)
        .push({
            let link = b.download_link(false);
            format!(
                "Download: [[Link]]({}) [[No Video]]({}?noVideo=1) [[Bloodcat]]({})",
                link,
                link,
                b.download_link(true),
            )
        })
        .push_line(format!(" [[Beatmapset]]({})", b.beatmapset_link()))
        .push("Language: ")
        .push_bold(&b.language)
        .push(" | Genre: ")
        .push_bold_line(&b.genre)
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

pub fn beatmap_embed<'a>(
    b: &'_ Beatmap,
    m: Mode,
    mods: Mods,
    info: Option<BeatmapInfoWithPP>,
    c: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let mod_str = if mods == Mods::NOMOD {
        "".to_owned()
    } else {
        format!(" {}", mods)
    };
    let diff = b
        .difficulty
        .apply_mods(mods, info.map(|(v, _)| v.stars as f64));
    c.title(
        MessageBuilder::new()
            .push_bold_safe(&b.artist)
            .push(" - ")
            .push_bold_safe(&b.title)
            .push(" [")
            .push_bold_safe(&b.difficulty_name)
            .push("]")
            .push(&mod_str)
            .build(),
    )
    .author(|a| {
        a.name(&b.creator)
            .url(format!("https://osu.ppy.sh/users/{}", b.creator_id))
            .icon_url(format!("https://a.ppy.sh/{}", b.creator_id))
    })
    .url(b.link())
    .image(b.cover_url())
    .color(0xffb6c1)
    .fields(info.map(|(_, pp)| {
        (
            "Calculated pp",
            format!(
                "95%: **{:.2}**pp, 98%: **{:.2}**pp, 99%: **{:.2}**pp, 100%: **{:.2}**pp",
                pp[0], pp[1], pp[2], pp[3]
            ),
            false,
        )
    }))
    .field("Information", diff.format_info(m, mods, b), false)
    .description(beatmap_description(b))
    .footer(|f| {
        if info.is_none() && mods != Mods::NOMOD {
            f.text("Star difficulty not reflecting mods applied.");
        }
        f
    })
}

const MAX_DIFFS: usize = 25 - 4;

pub fn beatmapset_embed<'a>(
    bs: &'_ [Beatmap],
    m: Option<Mode>,
    c: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let too_many_diffs = bs.len() > MAX_DIFFS;
    let b: &Beatmap = &bs[0];
    c.title(
        MessageBuilder::new()
            .push_bold_safe(&b.artist)
            .push(" - ")
            .push_bold_safe(&b.title)
            .build(),
    )
    .author(|a| {
        a.name(&b.creator)
            .url(format!("https://osu.ppy.sh/users/{}", b.creator_id))
            .icon_url(format!("https://a.ppy.sh/{}", b.creator_id))
    })
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
    .footer(|f| {
        if too_many_diffs {
            f.text(format!(
                "This map has {} diffs, we are showing the last {}.",
                bs.len(),
                MAX_DIFFS
            ))
        } else {
            f
        }
    })
    .fields(bs.iter().rev().take(MAX_DIFFS).rev().map(|b: &Beatmap| {
        (
            format!("[{}]", b.difficulty_name),
            b.difficulty
                .format_info(m.unwrap_or(b.mode), Mods::NOMOD, b),
            false,
        )
    }))
}

pub(crate) struct ScoreEmbedBuilder<'a> {
    s: &'a Score,
    bm: &'a BeatmapWithMode,
    content: &'a BeatmapContent,
    u: &'a User,
    top_record: Option<u8>,
    world_record: Option<u16>,
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
}

pub(crate) fn score_embed<'a>(
    s: &'a Score,
    bm: &'a BeatmapWithMode,
    content: &'a BeatmapContent,
    u: &'a User,
) -> ScoreEmbedBuilder<'a> {
    ScoreEmbedBuilder {
        s,
        bm,
        content,
        u,
        top_record: None,
        world_record: None,
    }
}

impl<'a> ScoreEmbedBuilder<'a> {
    #[allow(clippy::many_single_char_names)]
    pub fn build<'b>(&self, m: &'b mut CreateEmbed) -> &'b mut CreateEmbed {
        let mode = self.bm.mode();
        let b = &self.bm.0;
        let s = self.s;
        let content = self.content;
        let u = self.u;
        let accuracy = s.accuracy(mode);
        let info = mode
            .to_oppai_mode()
            .and_then(|mode| content.get_info_with(Some(mode), s.mods).ok());
        let stars = info
            .as_ref()
            .map(|info| info.stars as f64)
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
            mode.to_oppai_mode()
                .and_then(|op| {
                    content
                        .get_pp_from(
                            oppai_rs::Combo::non_fc(s.max_combo as u32, s.count_miss as u32),
                            OppaiAccuracy::from_hits(s.count_100 as u32, s.count_50 as u32),
                            Some(op),
                            s.mods,
                        )
                        .ok()
                })
                .map(|pp| (pp as f64, format!("{:.2}pp [?]", pp)))
        });
        let pp = if !s.perfect {
            mode.to_oppai_mode()
                .and_then(|op| {
                    content
                        .get_pp_from(
                            oppai_rs::Combo::FC(0),
                            OppaiAccuracy::from_hits(s.count_100 as u32, s.count_50 as u32),
                            Some(op),
                            s.mods,
                        )
                        .ok()
                })
                .filter(|&v| {
                    pp.as_ref()
                        .map(|&(origin, _)| origin < v as f64)
                        .unwrap_or(false)
                })
                .and_then(|value| {
                    pp.as_ref()
                        .map(|(_, original)| format!("{} ({:.2}pp if FC?)", original, value))
                })
                .or_else(|| pp.map(|v| v.1))
        } else {
            pp.map(|v| v.1)
        };
        let pp_gained = s.pp.map(|full_pp| {
            self.top_record
                .map(|top| {
                    let after_pp = u.pp.unwrap();
                    let effective_pp = full_pp * (0.95f64).powi(top as i32 - 1);
                    let before_pp = after_pp - effective_pp;
                    format!(
                        "**pp gained**: **{:.2}**pp (+**{:.2}**pp | {:.2}pp \\➡️ {:.2}pp)",
                        full_pp, effective_pp, before_pp, after_pp
                    )
                })
                .unwrap_or_else(|| format!("**pp gained**: **{:.2}**pp", full_pp))
        });
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
            .map(|v| format!("| #{} top record!", v))
            .unwrap_or_else(|| "".to_owned());
        let world_record = self
            .world_record
            .map(|v| format!("| #{} on Global Rankings!", v))
            .unwrap_or_else(|| "".to_owned());
        let diff = b.difficulty.apply_mods(s.mods, Some(stars));
        let creator = if b.difficulty_name.contains("'s") {
            "".to_owned()
        } else {
            format!("by {} ", b.creator)
        };
        m.author(|f| f.name(&u.username).url(u.link()).icon_url(u.avatar_url()))
            .color(0xffb6c1)
            .title(format!(
                "{} | {} - {} [{}] {} ({:.2}\\*) {}| {} {} {}",
                u.username,
                b.artist,
                b.title,
                b.difficulty_name,
                s.mods,
                stars,
                creator,
                score_line,
                top_record,
                world_record,
            ))
            .description(format!(
                r#"**Beatmap**: {} - {} [{}]**{} **
**Links**: [[Listing]]({}) [[Download]]({}) [[Bloodcat]]({})
**Played on**: {}
{}"#,
                b.artist,
                b.title,
                b.difficulty_name,
                s.mods,
                b.link(),
                b.download_link(false),
                b.download_link(true),
                s.date.format("%F %T"),
                pp_gained.as_ref().map(|v| &v[..]).unwrap_or(""),
            ))
            .image(b.cover_url())
            .field(
                "Score stats",
                format!(
                    "**{}** | {} | **{:.2}%**",
                    grouped_number(s.score),
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
            .field("Map stats", diff.format_info(mode, s.mods, b), false)
            .timestamp(&s.date);
        if mode.to_oppai_mode().is_none() && s.mods != Mods::NOMOD {
            m.footer(|f| f.text("Star difficulty does not reflect game mods."));
        }
        m
    }
}

pub(crate) fn user_embed(
    u: User,
    best: Option<(Score, BeatmapWithMode, Option<BeatmapInfo>)>,
    m: &mut CreateEmbed,
) -> &mut CreateEmbed {
    m.title(u.username)
        .url(format!("https://osu.ppy.sh/users/{}", u.id))
        .color(0xffb6c1)
        .thumbnail(format!("https://a.ppy.sh/{}", u.id))
        .description(format!("Member since **{}**", u.joined.format("%F %T")))
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
                "{} ({})",
                grouped_number(u.play_count),
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
                    .push_line(format!(
                        "{:.1} ago",
                        Duration(
                            (Utc::now() - v.date)
                                .to_std()
                                .unwrap_or_else(|_| std::time::Duration::from_secs(1))
                        )
                    ))
                    .push("on ")
                    .push_line(format!(
                        "[{} - {} [{}]]({})**{} **",
                        MessageBuilder::new().push_bold_safe(&map.artist).build(),
                        MessageBuilder::new().push_bold_safe(&map.title).build(),
                        map.difficulty_name,
                        map.link(),
                        v.mods
                    ))
                    .push(format!(
                        "{:.1}⭐ | `{}`",
                        info.map(|i| i.stars as f64).unwrap_or(map.difficulty.stars),
                        map.short_link(Some(mode), Some(v.mods))
                    ))
                    .build(),
                false,
            )
        }))
}
