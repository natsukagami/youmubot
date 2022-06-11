use crate::db::{ServerSoftBans, SoftBans};
use chrono::offset::Utc;
use futures_util::{stream, TryStreamExt};
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::Message,
        id::{GuildId, RoleId, UserId},
    },
    CacheAndHttp,
};
use std::sync::Arc;
use youmubot_prelude::*;

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Soft-ban an user, might be with a certain amount of time. Re-banning an user removes the ban itself."]
#[usage = "user#1234 [time]"]
#[example = "user#1234 5s"]
#[min_args(1)]
#[max_args(2)]
#[only_in("guilds")]
pub async fn soft_ban(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx).await?;
    let data = ctx.data.read().await;
    let duration = if args.is_empty() {
        None
    } else {
        Some(args.single::<args::Duration>()?)
    };
    let guild = msg
        .guild_id
        .ok_or_else(|| Error::msg("Command is guild only"))?;

    let mut db = SoftBans::open(&*data);
    let val = db
        .borrow()?
        .get(&guild)
        .map(|v| (v.role, v.periodical_bans.get(&user.id).cloned()));
    let (role, current_ban_deadline) = match val {
        None => {
            msg.reply(&ctx, "⚠ This server has not enabled the soft-ban feature. Check out `y!a soft-ban-init`.").await?;
            return Ok(());
        }
        Some(v) => v,
    };

    let mut member = guild.member(&ctx, &user).await?;
    match duration {
        None if member.roles.contains(&role) => {
            msg.reply(&ctx, format!("⛓ Lifting soft-ban for user {}.", user.tag()))
                .await?;
            member.remove_role(&ctx, role).await?;
            return Ok(());
        }
        None => {
            msg.reply(&ctx, format!("⛓ Soft-banning user {}.", user.tag()))
                .await?;
        }
        Some(v) => {
            // Add the duration into the ban timeout.
            let until =
                current_ban_deadline.unwrap_or_else(Utc::now) + chrono::Duration::from_std(v.0)?;
            msg.reply(
                &ctx,
                format!("⛓ Soft-banning user {} until {}.", user.tag(), until),
            )
            .await?;
            db.borrow_mut()?
                .get_mut(&guild)
                .map(|v| v.periodical_bans.insert(user.id, until));
        }
    }
    member.add_role(&ctx, role).await?;

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Sets up the soft-ban command. This command can only be run once.\nThe soft-ban command assigns a role, temporarily, to a user."]
#[usage = "{soft_ban_role_id}"]
#[num_args(1)]
#[only_in("guilds")]
pub async fn soft_ban_init(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let role_id = args.single::<RoleId>()?;
    let data = ctx.data.read().await;
    let guild = msg.guild(&ctx).unwrap();
    // Check whether the role_id is the one we wanted
    if !guild.roles.contains_key(&role_id) {
        return Err(Error::msg(format!("{} is not a role in this server.", role_id)).into());
    }
    // Check if we already set up
    let mut db = SoftBans::open(&*data);
    let set_up = db.borrow()?.contains_key(&guild.id);

    if !set_up {
        db.borrow_mut()?
            .insert(guild.id, ServerSoftBans::new(role_id));
        msg.react(&ctx, '👌').await?;
    } else {
        return Err(Error::msg("Server already set up soft-bans.").into());
    }
    Ok(())
}

// Watch the soft bans. Blocks forever.
pub async fn watch_soft_bans(cache_http: Arc<CacheAndHttp>, data: AppData) {
    loop {
        // Scope so that locks are released
        {
            // Poll the data for any changes.
            let data = data.read().await;
            let mut data = SoftBans::open(&*data);
            let mut db = data.borrow().unwrap().clone();
            let now = Utc::now();
            for (server_id, bans) in db.iter_mut() {
                let server_name: String = match server_id.to_partial_guild(&*cache_http.http).await
                {
                    Err(_) => continue,
                    Ok(v) => v.name,
                };
                let to_remove: Vec<_> = bans
                    .periodical_bans
                    .iter()
                    .filter_map(|(user, time)| if time <= &now { Some(user) } else { None })
                    .cloned()
                    .collect();
                if let Err(e) = to_remove
                    .into_iter()
                    .map(|user_id| {
                        bans.periodical_bans.remove(&user_id);
                        lift_soft_ban_for(
                            &*cache_http,
                            *server_id,
                            &server_name[..],
                            bans.role,
                            user_id,
                        )
                    })
                    .collect::<stream::FuturesUnordered<_>>()
                    .try_collect::<()>()
                    .await
                {
                    eprintln!("Error while scanning soft-bans list: {}", e)
                }
            }
            *(data.borrow_mut().unwrap()) = db;
        }
        // Sleep the thread for a minute
        tokio::time::sleep(std::time::Duration::from_secs(60)).await
    }
}

async fn lift_soft_ban_for(
    cache_http: &CacheAndHttp,
    server_id: GuildId,
    server_name: &str,
    ban_role: RoleId,
    user_id: UserId,
) -> Result<()> {
    let mut m = server_id.member(cache_http, user_id).await?;
    println!(
        "Soft-ban for `{}` in server `{}` unlifted.",
        m.user.name, server_name
    );
    m.remove_role(&cache_http.http, ban_role).await?;
    Ok(())
}
