use std::{collections::HashMap, sync::Arc, time::Duration};

use codeforces::Contest;
use pagination::paginate_from_fn;
use serenity::{
    builder::CreateMessage,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, guild::Member},
    utils::MessageBuilder,
};

use db::{CfSavedUsers, CfUser};
pub use hook::InfoHook;
use youmubot_prelude::announcer::AnnouncerHandler;
use youmubot_prelude::table_format::table_formatting_unsafe;
use youmubot_prelude::table_format::Align::{Left, Right};
use youmubot_prelude::{
    table_format::{table_formatting, Align},
    *,
};

mod announcer;
mod db;
mod embed;
mod hook;

/// Live-commentating a Codeforces round.
mod live;

/// The TypeMapKey holding the Client.
struct CFClient;

impl TypeMapKey for CFClient {
    type Value = Arc<codeforces::Client>;
}

/// Sets up the CF databases.
pub async fn setup(path: &std::path::Path, data: &mut TypeMap, announcers: &mut AnnouncerHandler) {
    CfSavedUsers::insert_into(data, path.join("cf_saved_users.yaml"))
        .expect("Must be able to set up DB");
    let client = Arc::new(codeforces::Client::new());
    data.insert::<hook::ContestCache>(hook::ContestCache::new(client.clone()).await.unwrap());
    data.insert::<CFClient>(client);
    data.insert::<live::WatchData>(live::WatchData::new());
    announcers.add("codeforces", announcer::Announcer);
}

#[group]
#[prefix = "cf"]
#[description = "Codeforces-related commands"]
#[commands(profile, save, ranks, watch, contestranks)]
#[default_command(profile)]
pub struct Codeforces;

#[command]
#[aliases("p", "show", "u", "user", "get")]
#[description = "Get an user's profile"]
#[usage = "[handle or tag = yourself]"]
#[example = "natsukagami"]
#[max_args(1)]
pub async fn profile(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let handle = args
        .single::<UsernameArg>()
        .unwrap_or_else(|_| UsernameArg::mention(m.author.id));
    let http = data.get::<CFClient>().unwrap();

    let handle = match handle {
        UsernameArg::Raw(s) => s,
        UsernameArg::Tagged(u) => {
            let db = CfSavedUsers::open(&data);
            let user = db.borrow()?.get(&u).map(|u| u.handle.clone());
            match user {
                Some(v) => v,
                None => {
                    m.reply(&ctx, "no saved account found.").await?;
                    return Ok(());
                }
            }
        }
    };

    let account = codeforces::User::info(http, &[&handle[..]])
        .await?
        .into_iter()
        .next();

    match account {
        Some(v) => {
            m.channel_id
                .send_message(&ctx, {
                    CreateMessage::new()
                        .content(format!(
                            "{}: Here is the user that you requested",
                            m.author.mention()
                        ))
                        .embed(embed::user_embed(&v))
                })
                .await
        }
        None => m.reply(&ctx, "User not found").await,
    }?;

    Ok(())
}

#[command]
#[description = "Link your Codeforces account to the Discord account, to enjoy Youmu's tracking capabilities."]
#[usage = "[handle]"]
#[num_args(1)]
pub async fn save(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let handle = args.single::<String>()?;
    let http = data.get::<CFClient>().unwrap();

    let account = codeforces::User::info(http, &[&handle[..]])
        .await?
        .into_iter()
        .next();

    match account {
        None => {
            m.reply(&ctx, "cannot find an account with such handle")
                .await?;
        }
        Some(acc) => {
            // Collect rating changes data.
            let rating_changes = acc.rating_changes(http).await?;
            let mut db = CfSavedUsers::open(&data);
            m.reply(
                &ctx,
                format!("account `{}` has been linked to your account.", &acc.handle),
            )
            .await?;
            db.borrow_mut()?
                .insert(m.author.id, CfUser::save(acc, rating_changes));
        }
    }

    Ok(())
}

#[command]
#[description = "See the leaderboard of all people in the server."]
#[only_in(guilds)]
#[num_args(0)]
pub async fn ranks(ctx: &Context, m: &Message) -> CommandResult {
    let data = ctx.data.read().await;
    let everyone = {
        let db = CfSavedUsers::open(&data);
        let db = db.borrow()?;
        db.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<_>>()
    };
    let guild = m.guild_id.expect("Guild-only command");
    let mut ranks = everyone
        .into_iter()
        .map(|(id, cf_user)| {
            guild
                .member(&ctx, id)
                .map(|mem| mem.map(|mem| (mem, cf_user)))
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| future::ready(v.ok()))
        .collect::<Vec<_>>()
        .await;
    ranks.sort_by(|(_, a), (_, b)| b.rating.unwrap_or(-1).cmp(&a.rating.unwrap_or(-1)));

    if ranks.is_empty() {
        m.reply(&ctx, "No saved users in this server.").await?;
        return Ok(());
    }

    let ranks = Arc::new(ranks);

    const ITEMS_PER_PAGE: usize = 10;
    let total_pages = (ranks.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let last_updated = ranks.iter().map(|(_, cfu)| cfu.last_update).min().unwrap();

    paginate_reply(
        paginate_from_fn(move |page, btns| {
            use Align::*;
            let ranks = ranks.clone();
            Box::pin(async move {
                let page = page as usize;
                let start = ITEMS_PER_PAGE * page;
                let end = ranks.len().min(start + ITEMS_PER_PAGE);
                if start >= end {
                    return Ok(None);
                }
                let ranks = &ranks[start..end];

                const HEADERS: [&str; 4] = ["Rank", "Rating", "Handle", "Username"];
                const ALIGNS: [Align; 4] = [Right, Right, Left, Left];

                let ranks_arr = ranks
                    .iter()
                    .enumerate()
                    .map(|(i, (mem, cfu))| {
                        [
                            format!("#{}", 1 + i + start),
                            cfu.rating
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "----".to_owned()),
                            cfu.handle.clone(),
                            mem.distinct(),
                        ]
                    })
                    .collect::<Vec<_>>();

                let table = table_formatting(&HEADERS, &ALIGNS, ranks_arr);

                let content = MessageBuilder::new()
                    .push_line(table)
                    .push_line(format!(
                        "Page **{}/{}**. Last updated **{}**",
                        page + 1,
                        total_pages,
                        last_updated.to_rfc2822()
                    ))
                    .build();

                Ok(Some(
                    CreateReply::default().content(content).components(btns),
                ))
            })
        })
        .with_page_count(total_pages),
        ctx,
        m,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}

#[command]
#[description = "See the server ranks on a certain contest"]
#[usage = "[the contest id]"]
#[num_args(1)]
#[only_in(guilds)]
pub async fn contestranks(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let contest_id: u64 = args.single()?;
    let guild = m.guild_id.unwrap(); // Guild-only command
    let member_cache = data.get::<MemberCache>().unwrap();
    let members = CfSavedUsers::open(&data).borrow()?.clone();
    let members = members
        .into_iter()
        .map(|(user_id, cf_user)| {
            member_cache
                .query(&ctx, user_id, guild)
                .map(|v| v.map(|v| (cf_user.handle, v)))
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(future::ready)
        .collect::<HashMap<_, _>>()
        .await;
    let http = data.get::<CFClient>().unwrap();
    let (contest, problems, ranks) = Contest::standings(http, contest_id, |f| {
        f.handles(members.keys().cloned().collect())
    })
    .await?;

    // Table me
    let ranks = ranks
        .into_iter()
        .flat_map(|v| {
            v.party
                .members
                .iter()
                .filter_map(|m| {
                    members
                        .get(&m.handle)
                        .cloned()
                        .map(|mem| (mem, m.handle.clone(), v.clone()))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    contest_rank_table(ctx, m, contest, problems, ranks).await?;

    Ok(())
}

#[command]
#[description = "Watch a contest and announce any change on the members of the server assigned to the contest."]
#[usage = "[the contest id]"]
#[num_args(1)]
#[required_permissions(MANAGE_CHANNELS)]
#[only_in(guilds)]
pub async fn watch(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let contest_id: u64 = args.single()?;

    live::watch_contest(ctx, m.guild_id.unwrap(), m.channel_id, contest_id).await?;

    Ok(())
}

pub(crate) async fn contest_rank_table(
    ctx: &Context,
    reply_to: &Message,
    contest: Contest,
    problems: Vec<codeforces::Problem>,
    ranks: Vec<(Member, String, codeforces::RanklistRow)>,
) -> Result<()> {
    const ITEMS_PER_PAGE: usize = 10;
    let total_pages = (ranks.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;

    if ranks.is_empty() {
        reply_to
            .reply(&ctx, "No one in this server participated in the contest...")
            .await?;
        return Ok(());
    }
    let ranks = Arc::new(ranks);

    paginate_reply(
        paginate_from_fn(move |page, btns| {
            let contest = contest.clone();
            let problems = problems.clone();
            let ranks = ranks.clone();
            Box::pin(async move {
                let page = page as usize;
                let start = page * ITEMS_PER_PAGE;
                let end = ranks.len().min(start + ITEMS_PER_PAGE);
                if start >= end {
                    return Ok(None);
                }
                let ranks = &ranks[start..end];

                let score_headers: Vec<&str> = [
                    vec!["Rank", "Handle", "User", "Total", "Hacks"],
                    problems
                        .iter()
                        .map(|p| p.index.as_str())
                        .collect::<Vec<&str>>(),
                ]
                .concat();

                let score_aligns: Vec<Align> = [
                    vec![Right, Left, Left, Right, Right],
                    problems.iter().map(|_| Right).collect::<Vec<Align>>(),
                ]
                .concat();

                let score_arr = ranks
                    .iter()
                    .map(|(mem, handle, row)| {
                        let mut p_results: Vec<String> = Vec::new();
                        for result in &row.problem_results {
                            if result.points > 0.0 {
                                p_results.push(format!("{}", result.points));
                            } else if result.best_submission_time_seconds.is_some() {
                                p_results.push("?".to_string());
                            } else if result.rejected_attempt_count > 0 {
                                p_results.push(format!("-{}", result.rejected_attempt_count));
                            } else {
                                p_results.push("----".to_string());
                            }
                        }

                        [
                            vec![
                                format!("{}", row.rank),
                                handle.clone(),
                                mem.distinct(),
                                format!("{}", row.points),
                                format!(
                                    "{}/{}",
                                    row.successful_hack_count, row.unsuccessful_hack_count
                                ),
                            ],
                            p_results,
                        ]
                        .concat()
                    })
                    .collect::<Vec<_>>();

                let score_table = table_formatting_unsafe(&score_headers, &score_aligns, score_arr);

                let content = MessageBuilder::new()
                    .push_bold_safe(&contest.name)
                    .push(" ")
                    .push_line(contest.url())
                    .push_line(score_table)
                    .push_line(format!("Page **{}/{}**", page + 1, total_pages))
                    .build();

                Ok(Some(
                    CreateReply::default().content(content).components(btns),
                ))
            })
        })
        .with_page_count(total_pages),
        ctx,
        reply_to,
        Duration::from_secs(60),
    )
    .await
}
