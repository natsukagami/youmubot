use crate::{
    commands::args,
    db::{DBWriteGuard, ServerSoftBans, SoftBans},
};
use chrono::offset::Utc;
use serenity::prelude::*;
use serenity::{
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    model::{
        channel::Message,
        id::{RoleId, UserId},
    },
};
use std::cmp::max;

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Soft-ban an user, might be with a certain amount of time. Re-banning an user removes the ban itself."]
#[usage = "user#1234 [time]"]
#[example = "user#1234 5s"]
#[min_args(1)]
#[max_args(2)]
pub fn soft_ban(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx)?;
    let duration = if args.is_empty() {
        None
    } else {
        Some(
            args.single::<args::Duration>()
                .map_err(|e| Error::from(&format!("{:?}", e)))?,
        )
    };
    let guild = msg.guild_id.ok_or(Error::from("Command is guild only"))?;

    let mut data = ctx.data.write();
    let mut data = data
        .get_mut::<SoftBans>()
        .ok_or(Error::from("DB initialized"))
        .map(|v| DBWriteGuard::from(v))?;
    let mut data = data.borrow_mut()?;
    let mut server_ban = data.get_mut(&guild).and_then(|v| match v {
        ServerSoftBans::Unimplemented => None,
        ServerSoftBans::Implemented(ref mut v) => Some(v),
    });

    match server_ban {
        None => {
            println!("get here");
            msg.reply(&ctx, format!("⚠ This server has not enabled the soft-ban feature. Check out `y!a soft-ban-init`."))?;
        }
        Some(ref mut server_ban) => {
            let mut member = guild.member(&ctx, &user)?;
            match duration {
                None if member.roles.contains(&server_ban.role) => {
                    msg.reply(&ctx, format!("⛓ Lifting soft-ban for user {}.", user.tag()))?;
                    member.remove_role(&ctx, server_ban.role)?;
                    return Ok(());
                }
                None => {
                    msg.reply(&ctx, format!("⛓ Soft-banning user {}.", user.tag()))?;
                }
                Some(v) => {
                    let until = Utc::now() + v.0;
                    let until = server_ban
                        .periodical_bans
                        .entry(user.id)
                        .and_modify(|v| *v = max(*v, until))
                        .or_insert(until);
                    msg.reply(
                        &ctx,
                        format!("⛓ Soft-banning user {} until {}.", user.tag(), until),
                    )?;
                }
            }
            member.add_role(&ctx, server_ban.role)?;
        }
    }

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Sets up the soft-ban command. This command can only be run once.\nThe soft-ban command assigns a role, temporarily, to a user."]
#[usage = "{soft_ban_role_id}"]
#[num_args(1)]
pub fn soft_ban_init(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let role_id = args.single::<RoleId>()?;
    let guild = msg.guild(&ctx).ok_or(Error::from("Guild-only command"))?;
    let guild = guild.read();
    // Check whether the role_id is the one we wanted
    if !guild.roles.contains_key(&role_id) {
        return Err(Error::from(format!(
            "{} is not a role in this server.",
            role_id
        )));
    }
    // Check if we already set up
    let mut data = ctx.data.write();
    let mut db: DBWriteGuard<_> = data
        .get_mut::<SoftBans>()
        .ok_or(Error::from("DB uninitialized"))?
        .into();
    let mut db = db.borrow_mut()?;
    let server = db
        .get_mut(&guild.id)
        .map(|v| match v {
            ServerSoftBans::Unimplemented => false,
            _ => true,
        })
        .unwrap_or(false);

    if !server {
        db.insert(guild.id, ServerSoftBans::new_implemented(role_id));
        msg.react(&ctx, "👌")?;
        Ok(())
    } else {
        Err(Error::from("Server already set up soft-bans."))
    }
}

// Watch the soft bans.
pub fn watch_soft_bans(client: &mut serenity::Client) -> impl FnOnce() -> () + 'static {
    let cache_http = {
        let cache_http = client.cache_and_http.clone();
        let cache: serenity::cache::CacheRwLock = cache_http.cache.clone().into();
        (cache, cache_http.http.clone())
    };
    let data = client.data.clone();
    return move || {
        let cache_http = (&cache_http.0, &*cache_http.1);
        loop {
            // Scope so that locks are released
            {
                // Poll the data for any changes.
                let mut data = data.write();
                let mut db: DBWriteGuard<_> = data
                    .get_mut::<SoftBans>()
                    .expect("DB wrongly initialized")
                    .into();
                let mut db = db.borrow_mut().expect("cannot unpack DB");
                let now = Utc::now();
                for (server_id, soft_bans) in db.iter_mut() {
                    let server_name: String = match server_id.to_partial_guild(cache_http) {
                        Err(_) => continue,
                        Ok(v) => v.name,
                    };
                    if let ServerSoftBans::Implemented(ref mut bans) = soft_bans {
                        let to_remove: Vec<_> = bans
                            .periodical_bans
                            .iter()
                            .filter_map(|(user, time)| if time <= &now { Some(user) } else { None })
                            .cloned()
                            .collect();
                        for user_id in to_remove {
                            server_id
                                .member(cache_http, user_id)
                                .and_then(|mut m| {
                                    println!(
                                        "Soft-ban for `{}` in server `{}` unlifted.",
                                        m.user.read().name,
                                        server_name
                                    );
                                    m.remove_role(cache_http, bans.role)
                                })
                                .unwrap_or(());
                            bans.periodical_bans.remove(&user_id);
                        }
                    }
                }
            }
            // Sleep the thread for a minute
            std::thread::sleep(std::time::Duration::from_secs(60))
        }
    };
}