use crate::db::{CfSavedUsers, CfUser};
use announcer::MemberToChannels;
use chrono::{DateTime, Utc};
use codeforces::{RatingChange, User};
use serenity::{
    framework::standard::{CommandError, CommandResult},
    http::CacheHttp,
    model::id::{ChannelId, UserId},
    CacheAndHttp,
};
use std::sync::Arc;
use youmubot_prelude::*;

type Reqwest = <HTTPClient as TypeMapKey>::Value;

/// Updates the rating and rating changes of the users.
pub fn updates(
    http: Arc<CacheAndHttp>,
    data: AppData,
    channels: MemberToChannels,
) -> CommandResult {
    let mut users = CfSavedUsers::open(&*data.read()).borrow()?.clone();
    let reqwest = data.get_cloned::<HTTPClient>();

    for (user_id, cfu) in users.iter_mut() {
        if let Err(e) = update_user(http.clone(), &channels, &reqwest, *user_id, cfu) {
            dbg!((*user_id, e));
        }
    }

    *CfSavedUsers::open(&*data.read()).borrow_mut()? = users;
    Ok(())
}

fn update_user(
    http: Arc<CacheAndHttp>,
    channels: &MemberToChannels,
    reqwest: &Reqwest,
    user_id: UserId,
    cfu: &mut CfUser,
) -> CommandResult {
    let info = User::info(reqwest, &[cfu.handle.as_str()])?
        .into_iter()
        .next()
        .ok_or(CommandError::from("Not found"))?;

    let rating_changes = {
        let mut v = info.rating_changes(reqwest)?;
        v.reverse();
        v
    };

    let mut channels_list: Option<Vec<ChannelId>> = None;
    let last_update = std::mem::replace(&mut cfu.last_update, Utc::now());
    // Update the rating
    cfu.rating = info.rating;

    let mut send_message = |rc: RatingChange| -> CommandResult {
        let (contest, _, _) =
            codeforces::Contest::standings(reqwest, rc.contest_id, |f| f.limit(1, 1))?;
        let channels =
            channels_list.get_or_insert_with(|| channels.channels_of(http.clone(), user_id));
        for channel in channels {
            if let Err(e) = channel.send_message(http.http(), |e| {
                e.content(format!("Rating change for {}!", user_id.mention()))
                    .embed(|c| {
                        crate::embed::rating_change_embed(
                            &rc,
                            &info,
                            &contest,
                            &user_id.mention(),
                            c,
                        )
                    })
            }) {
                dbg!(e);
            }
        }
        Ok(())
    };

    // Check for any good announcements to make
    for rc in rating_changes {
        let date: DateTime<Utc> = DateTime::from_utc(
            chrono::NaiveDateTime::from_timestamp(rc.rating_update_time_seconds as i64, 0),
            Utc,
        );
        if &date > &last_update {
            if let Err(v) = send_message(rc) {
                dbg!(v);
            }
        }
    }

    Ok(())
}
