use super::BeatmapWithMode;
use crate::models::{Beatmap, Mode, Rank, Score, User};
use chrono::Utc;
use serenity::{builder::CreateEmbed, utils::MessageBuilder};
use youmubot_prelude::*;

fn format_mode(actual: Mode, original: Mode) -> String {
    if actual == original {
        format!("{}", actual)
    } else {
        format!("{} (converted)", actual)
    }
}

pub fn beatmap_embed<'a>(b: &'_ Beatmap, m: Mode, c: &'a mut CreateEmbed) -> &'a mut CreateEmbed {
    c.title(
        MessageBuilder::new()
            .push_bold_safe(&b.artist)
            .push(" - ")
            .push_bold_safe(&b.title)
            .push(" [")
            .push_bold_safe(&b.difficulty_name)
            .push("]")
            .build(),
    )
    .author(|a| {
        a.name(&b.creator)
            .url(format!("https://osu.ppy.sh/users/{}", b.creator_id))
            .icon_url(format!("https://a.ppy.sh/{}", b.creator_id))
    })
    .url(b.link())
    .thumbnail(format!("https://b.ppy.sh/thumb/{}l.jpg", b.beatmapset_id))
    .image(b.cover_url())
    .color(0xffb6c1)
    .field(
        "Star Difficulty",
        format!("{:.2}⭐", b.difficulty.stars),
        false,
    )
    .field(
        "Length",
        MessageBuilder::new()
            .push_bold_safe(Duration(b.total_length))
            .push(" (")
            .push_bold_safe(Duration(b.drain_length))
            .push(" drain)")
            .build(),
        false,
    )
    .field("Circle Size", format!("{:.1}", b.difficulty.cs), true)
    .field("Approach Rate", format!("{:.1}", b.difficulty.ar), true)
    .field(
        "Overall Difficulty",
        format!("{:.1}", b.difficulty.od),
        true,
    )
    .field("HP Drain", format!("{:.1}", b.difficulty.hp), true)
    .field("BPM", b.bpm.round(), true)
    .fields(b.difficulty.max_combo.map(|v| ("Max combo", v, true)))
    .field("Mode", format_mode(m, b.mode), true)
    .fields(b.source.as_ref().map(|v| ("Source", v, true)))
    .field(
        "Tags",
        b.tags
            .iter()
            .map(|v| MessageBuilder::new().push_mono_safe(v).build())
            .take(10)
            .chain(std::iter::once("...".to_owned()))
            .collect::<Vec<_>>()
            .join(" "),
        false,
    )
    .description(
        MessageBuilder::new()
            .push({
                let link = format!(
                    "https://osu.ppy.sh/beatmapsets/{}/download",
                    b.beatmapset_id
                );
                format!(
                    "Download: [[Link]]({}) [[No Video]]({}?noVideo=1)",
                    link, link
                )
            })
            .push_line(format!(" [[Beatmapset]]({})", b.beatmapset_link()))
            .push_line(&b.approval)
            .push("Language: ")
            .push_bold(&b.language)
            .push(" | Genre: ")
            .push_bold(&b.genre)
            .build(),
    )
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
    // .thumbnail(format!("https://b.ppy.sh/thumb/{}l.jpg", b.beatmapset_id))
    .image(format!(
        "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
        b.beatmapset_id
    ))
    .color(0xffb6c1)
    .description(
        MessageBuilder::new()
            .push_line({
                let link = format!(
                    "https://osu.ppy.sh/beatmapsets/{}/download",
                    b.beatmapset_id
                );
                format!(
                    "Download: [[Link]]({}) [[No Video]]({}?noVideo=1)",
                    link, link
                )
            })
            .push_line(&b.approval)
            .push("Language: ")
            .push_bold(&b.language)
            .push(" | Genre: ")
            .push_bold(&b.genre)
            .build(),
    )
    .field(
        "Length",
        MessageBuilder::new()
            .push_bold_safe(Duration(b.total_length))
            .build(),
        true,
    )
    .field("BPM", b.bpm.round(), true)
    .fields(b.source.as_ref().map(|v| ("Source", v, false)))
    .field(
        "Tags",
        b.tags
            .iter()
            .map(|v| MessageBuilder::new().push_mono_safe(v).build())
            .take(10)
            .chain(std::iter::once("...".to_owned()))
            .collect::<Vec<_>>()
            .join(" "),
        false,
    )
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
            MessageBuilder::new()
                .push(format!("[[Link]]({})", b.link()))
                .push(", ")
                .push_bold(format!("{:.2}⭐", b.difficulty.stars))
                .push(", ")
                .push_bold_line(format_mode(m.unwrap_or(b.mode), b.mode))
                .push("CS")
                .push_bold(format!("{:.1}", b.difficulty.cs))
                .push(", AR")
                .push_bold(format!("{:.1}", b.difficulty.ar))
                .push(", OD")
                .push_bold(format!("{:.1}", b.difficulty.od))
                .push(", HP")
                .push_bold(format!("{:.1}", b.difficulty.hp))
                .push(", ⌛ ")
                .push_bold(format!("{}", Duration(b.drain_length)))
                .build(),
            false,
        )
    }))
}

pub(crate) fn score_embed<'a>(
    s: &Score,
    bm: &BeatmapWithMode,
    u: &User,
    top_record: Option<u8>,
    m: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let mode = bm.mode();
    let b = &bm.0;
    let accuracy = s.accuracy(mode);
    let score_line = match &s.rank {
        Rank::SS | Rank::SSH => format!("SS"),
        _ if s.perfect => format!("{:.2}% FC", accuracy),
        Rank::F => format!("{:.2}% {} combo [FAILED]", accuracy, s.max_combo),
        v => format!(
            "{:.2}% {}x {} miss {} rank",
            accuracy, s.max_combo, s.count_miss, v
        ),
    };
    let score_line =
        s.pp.map(|pp| format!("{} | {:.2}pp", &score_line, pp))
            .unwrap_or(score_line);
    let top_record = top_record
        .map(|v| format!("| #{} top record!", v))
        .unwrap_or("".to_owned());
    m.author(|f| f.name(&u.username).url(u.link()).icon_url(u.avatar_url()))
        .color(0xffb6c1)
        .title(format!(
            "{} | {} - {} [{}] {} ({:.2}\\*) by {} | {} {}",
            u.username,
            b.artist,
            b.title,
            b.difficulty_name,
            s.mods,
            b.difficulty.stars,
            b.creator,
            score_line,
            top_record
        ))
        .description(format!("[[Beatmap]]({})", b.link()))
        .image(b.cover_url())
        .field(
            "Beatmap",
            format!("{} - {} [{}]", b.artist, b.title, b.difficulty_name),
            false,
        )
        .field("Rank", &score_line, false)
        .fields(s.pp.map(|pp| ("pp gained", format!("{:.2}pp", pp), true)))
        .field("Creator", &b.creator, true)
        .field("Mode", mode.to_string(), true)
        .field(
            "Map stats",
            MessageBuilder::new()
                .push(format!("[[Link]]({})", b.link()))
                .push(", ")
                .push_bold(format!("{:.2}⭐", b.difficulty.stars))
                .push(", ")
                .push_bold_line(
                    b.mode.to_string()
                        + if bm.is_converted() {
                            " (Converted)"
                        } else {
                            ""
                        },
                )
                .push("CS")
                .push_bold(format!("{:.1}", b.difficulty.cs))
                .push(", AR")
                .push_bold(format!("{:.1}", b.difficulty.ar))
                .push(", OD")
                .push_bold(format!("{:.1}", b.difficulty.od))
                .push(", HP")
                .push_bold(format!("{:.1}", b.difficulty.hp))
                .push(", ⌛ ")
                .push_bold(format!("{}", Duration(b.drain_length)))
                .build(),
            false,
        )
        .field("Played on", s.date.format("%F %T"), false)
}

pub(crate) fn user_embed<'a>(
    u: User,
    best: Option<(Score, BeatmapWithMode)>,
    m: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    m.title(u.username)
        .url(format!("https://osu.ppy.sh/users/{}", u.id))
        .color(0xffb6c1)
        .thumbnail(format!("https://a.ppy.sh/{}", u.id))
        .description(format!("Member since **{}**", u.joined.format("%F %T")))
        .field(
            "Performance Points",
            u.pp.map(|v| format!("{:.2}pp", v))
                .unwrap_or("Inactive".to_owned()),
            false,
        )
        .field("World Rank", format!("#{}", u.rank), true)
        .field(
            "Country Rank",
            format!(":flag_{}: #{}", u.country.to_lowercase(), u.country_rank),
            true,
        )
        .field("Accuracy", format!("{:.2}%", u.accuracy), true)
        .field(
            "Play count",
            format!("{} (play time: {})", u.play_count, Duration(u.played_time)),
            false,
        )
        .field(
            "Ranks",
            format!(
                "{} SSH | {} SS | {} SH | {} S | {} A",
                u.count_ssh, u.count_ss, u.count_sh, u.count_s, u.count_a
            ),
            false,
        )
        .field(
            "Level",
            format!(
                "Level **{:.0}**: {} total score, {} ranked score",
                u.level, u.total_score, u.ranked_score
            ),
            false,
        )
        .fields(best.map(|(v, map)| {
            let map = map.0;
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
                                .unwrap_or(std::time::Duration::from_secs(1))
                        )
                    ))
                    .push("on ")
                    .push(format!(
                        "[{} - {}]({})",
                        MessageBuilder::new().push_bold_safe(&map.artist).build(),
                        MessageBuilder::new().push_bold_safe(&map.title).build(),
                        map.link()
                    ))
                    .push(format!(" [{}]", map.difficulty_name))
                    .push(format!(" ({:.1}⭐)", map.difficulty.stars))
                    .build(),
                false,
            )
        }))
}
