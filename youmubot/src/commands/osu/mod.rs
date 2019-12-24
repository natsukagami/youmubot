use crate::commands::args::Duration;
use crate::http;
use lazy_static::lazy_static;
use regex::Regex;
use serenity::{
    builder::CreateEmbed,
    framework::standard::{macros::group, CommandResult},
    model::{channel::Message, id::ChannelId},
    prelude::*,
    utils::MessageBuilder,
};
use youmubot_osu::{
    models::{Beatmap, Mode},
    request::BeatmapRequestKind,
};

group!({
    name: "osu",
    options: {
        prefix: "osu",
        description: "osu! related commands.",
    },
    commands: [],
});

lazy_static! {
    static ref OLD_LINK_REGEX: Regex = Regex::new(
        r"https?://osu\.ppy\.sh/(?P<link_type>s|b)/(?P<id>\d+)(?:[\&\?]m=(?P<mode>\d))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
    static ref NEW_LINK_REGEX: Regex = Regex::new(
        r"https?://osu\.ppy\.sh/beatmapsets/(?P<set_id>\d+)/?(?:\#(?P<mode>osu|taiko|fruits|mania)(?:/(?P<beatmap_id>\d+)|/?))?(?:\+(?P<mods>[A-Z]+))?"
    ).unwrap();
}

pub fn hook(ctx: &mut Context, msg: &Message) -> () {
    Some(msg)
        .filter(|&m| !m.author.bot) // Don't react to bot messages
        .map(|m| {
            if let Err(v) = handle_old_links(ctx, &m.content, m.channel_id) {
                println!("Error on old link handling: {:?}", v);
            }
        });
}

fn handle_old_links(ctx: &mut Context, content: impl AsRef<str>, ch: ChannelId) -> CommandResult {
    let data = ctx.data.write();
    let reqwest = data.get::<http::HTTP>().unwrap();
    let osu = data.get::<http::Osu>().unwrap();
    for capture in OLD_LINK_REGEX.captures_iter(content.as_ref()) {
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
        let mut beatmaps = osu.beatmaps(reqwest, req, |v| match mode {
            Some(m) => v.mode(m, true),
            None => v,
        })?;
        match req_type {
            "b" => {
                for beatmap in beatmaps.iter() {
                    if let Err(v) = ch.send_message(&ctx, |m| {
                        m.content(
                            MessageBuilder::new()
                                .push("Beatmap information for ")
                                .push_mono_safe(&capture[0])
                                .build(),
                        )
                        .embed(|b| beatmap_embed(beatmap, mode.unwrap_or(beatmap.mode), b))
                    }) {
                        println!("Error on printing beatmap {}: {:?}", beatmap.beatmap_id, v);
                    }
                }
            }
            "s" => {
                beatmaps.sort_by(|a, b| {
                    (mode.unwrap_or(a.mode) as u8, a.difficulty.stars)
                        .partial_cmp(&(mode.unwrap_or(b.mode) as u8, b.difficulty.stars))
                        .unwrap()
                });
                ch.send_message(&ctx, |m| {
                    m.content(
                        MessageBuilder::new()
                            .push("Beatmapset information for ")
                            .push_mono_safe(&capture[0])
                            .build(),
                    )
                    .embed(|b| beatmapset_embed(&beatmaps, mode, b))
                })?;
            }
            _ => (),
        }
    }
    Ok(())
}

const NEW_MODE_NAMES: [&'static str; 4] = ["osu", "taiko", "fruits", "mania"];

fn format_mode(actual: Mode, original: Mode) -> String {
    if actual == original {
        format!("{}", actual)
    } else {
        format!("{} (converted)", actual)
    }
}

fn beatmap_embed<'a>(b: &'_ Beatmap, m: Mode, c: &'a mut CreateEmbed) -> &'a mut CreateEmbed {
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
    .url(format!(
        "https://osu.ppy.sh/beatmapsets/{}/#{}/{}",
        b.beatmapset_id, NEW_MODE_NAMES[b.mode as usize], b.beatmap_id
    ))
    .thumbnail(format!("https://b.ppy.sh/thumb/{}l.jpg", b.beatmapset_id))
    .image(format!(
        "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
        b.beatmapset_id
    ))
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
            .push_line({
                let link = format!("https://osu.ppy.sh/beatmapsets/{}/download", b.beatmap_id);
                format!(
                    "Download: [[Link]]({}) [[No Video]]({}?noVideo=1)",
                    link, link
                )
            })
            .push_line(format!(
                "Beatmapset: https://osu.ppy.sh/beatmapsets/{}/#{}",
                b.beatmapset_id, NEW_MODE_NAMES[b.mode as usize],
            ))
            .build(),
    )
}

const MAX_DIFFS: usize = 25 - 4;

fn beatmapset_embed<'a>(
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
                let link = format!("https://osu.ppy.sh/beatmapsets/{}/download", b.beatmap_id);
                format!(
                    "Download: [[Link]]({}) [[No Video]]({}?noVideo=1)",
                    link, link
                )
            })
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
                "This map has {} diffs, we are showing the first {}.",
                bs.len(),
                MAX_DIFFS
            ))
        } else {
            f
        }
    })
    .fields(bs.iter().take(MAX_DIFFS).map(|b: &Beatmap| {
        (
            format!("[{}]", b.difficulty_name),
            MessageBuilder::new()
                .push(format!(
                    "[[Link]](https://osu.ppy.sh/beatmapsets/{}/#{}/{})",
                    b.beatmapset_id,
                    NEW_MODE_NAMES[m.unwrap_or(b.mode) as usize],
                    b.beatmap_id
                ))
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
