use codeforces::{Contest, RatingChange, User};
use serenity::{builder::CreateEmbed, utils::MessageBuilder};
use std::borrow::Borrow;

fn unwrap_or_ref<'a, T: ?Sized, B: Borrow<T>>(opt: &'a Option<B>, default: &'a T) -> &'a T {
    opt.as_ref().map(|v| v.borrow()).unwrap_or(default)
}

/// Create an embed representing the user.
pub fn user_embed<'a>(user: &User, e: &'a mut CreateEmbed) -> &'a mut CreateEmbed {
    let rank = unwrap_or_ref(&user.rank, "Unranked");
    let max_rank = unwrap_or_ref(&user.max_rank, "Unranked");
    let rating = user.rating.unwrap_or(1500);
    let max_rating = user.max_rating.unwrap_or(1500);
    let name = &[&user.first_name, &user.last_name]
        .iter()
        .filter_map(|v| v.as_ref().map(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(" ");
    let place = &[&user.organization, &user.city, &user.country]
        .iter()
        .filter_map(|v| v.as_ref().map(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(" ");
    e.color(user.color())
        .author(|a| a.name(rank))
        .title(&user.handle)
        .description(format!(
            "{}\n{}",
            if name == "" {
                "".to_owned()
            } else {
                format!("**{}**", name)
            },
            if place == "" {
                "".to_owned()
            } else {
                format!("from **{}**", place)
            }
        ))
        .field(
            "Rating",
            format!("**{}** (max **{}**)", rating, max_rating),
            true,
        )
        .field("Contribution", format!("**{}**", user.contribution), true)
        .field(
            "Rank",
            format!("**{}** (max **{}**)", rank, max_rank),
            false,
        )
}

/// Gets an embed of the Rating Change.
pub fn rating_change_embed<'a>(
    rating_change: &RatingChange,
    user: &User,
    contest: &Contest,
    tag: &str,
    e: &'a mut CreateEmbed,
) -> &'a mut CreateEmbed {
    let delta = (rating_change.new_rating as i64) - (rating_change.old_rating as i64);
    let color = if delta < 0 { 0xff0000 } else { 0x00ff00 };
    let message = if delta < 0 {
        MessageBuilder::new()
            .push(tag)
            .push(" competed in ")
            .push_bold_safe(&contest.name)
            .push(", gaining ")
            .push_bold_safe(delta)
            .push(" rating placing at ")
            .push_bold(format!("#{}", rating_change.rank))
            .push("! 🎂🎂🎂")
            .build()
    } else {
        MessageBuilder::new()
            .push(tag)
            .push(" competed in ")
            .push_bold_safe(&contest.name)
            .push(", but lost ")
            .push_bold_safe(-delta)
            .push(" rating placing at ")
            .push_bold(format!("#{}", rating_change.rank))
            .push("... 😭😭😭")
            .build()
    };

    e.author(|a| {
        a.icon_url(&user.avatar)
            .url(user.profile_url())
            .name(&user.handle)
    })
    .color(color)
    .description(message)
    .field("Contest Link", contest.url(), true)
}
