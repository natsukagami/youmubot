use codeforces::{Contest, RatingChange, User};
use inflector::Inflector;
use serenity::{
    builder::{CreateEmbed, CreateEmbedAuthor},
    utils::MessageBuilder,
};
use std::borrow::Borrow;
use youmubot_prelude::*;

fn unwrap_or_ref<'a, T: ?Sized, B: Borrow<T>>(opt: &'a Option<B>, default: &'a T) -> &'a T {
    opt.as_ref().map(|v| v.borrow()).unwrap_or(default)
}

/// Create an embed representing the user.
pub fn user_embed<'a>(user: &User) -> CreateEmbed {
    let rank = unwrap_or_ref(&user.rank, "Unranked").to_title_case();
    let max_rank = unwrap_or_ref(&user.max_rank, "Unranked").to_title_case();
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
        .join(", ");
    CreateEmbed::new()
        .color(user.color())
        .author(CreateEmbedAuthor::new(&rank))
        .thumbnail(user.title_photo.to_string())
        .title(&user.handle)
        .url(user.profile_url())
        .description(format!(
            "{}\n{}",
            if name.is_empty() {
                "".to_owned()
            } else {
                format!("**{}**", name)
            },
            if place.is_empty() {
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
            format!("**{}** (max **{}**)", &rank, max_rank),
            false,
        )
}

/// Gets an embed of the Rating Change.
pub fn rating_change_embed<'a>(
    rating_change: &RatingChange,
    user: &User,
    contest: &Contest,
    user_id: serenity::model::id::UserId,
) -> CreateEmbed {
    let delta = rating_change.new_rating - rating_change.old_rating;
    let color = if delta < 0 { 0xff0000 } else { 0x00ff00 };
    let message = if delta > 0 {
        MessageBuilder::new()
            .push(user_id.mention().to_string())
            .push(" competed in ")
            .push_bold_safe(&contest.name)
            .push(", gaining ")
            .push_bold_safe(delta.to_string())
            .push(" rating placing at ")
            .push_bold(format!("#{}", rating_change.rank))
            .push("! 🎂🎂🎂")
            .build()
    } else {
        MessageBuilder::new()
            .push(user_id.mention().to_string())
            .push(" competed in ")
            .push_bold_safe(&contest.name)
            .push(", but lost ")
            .push_bold_safe((-delta).to_string())
            .push(" rating placing at ")
            .push_bold(format!("#{}", rating_change.rank))
            .push("... 😭😭😭")
            .build()
    };

    CreateEmbed::new()
        .author({
            CreateEmbedAuthor::new(&user.handle)
                .icon_url(user.avatar.to_string())
                .url(user.profile_url())
        })
        .color(color)
        .description(message)
        .field("Contest Link", contest.url(), true)
        .field(
            "Rating Change",
            format!(
                "from **{}** to **{}**",
                rating_change.old_rating, rating_change.new_rating
            ),
            false,
        )
}
