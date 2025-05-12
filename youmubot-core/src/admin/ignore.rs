use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serenity::all::User;
use youmubot_db_sql::models::ignore_list as model;
use youmubot_prelude::*;

use crate::HasCoreEnv;

// Should we ignore this user?
pub fn should_ignore(env: &impl HasCoreEnv, id: UserId) -> bool {
    env.core_env().ignore.query(id).is_some()
}

/// Ignore: make Youmu ignore all commands from an user.
#[poise::command(
    slash_command,
    subcommands("add", "remove", "list"),
    owners_only,
    install_context = "User",
    interaction_context = "Guild|BotDm"
)]
pub async fn ignore<U: HasCoreEnv>(_ctx: CmdContext<'_, U>) -> Result<()> {
    Ok(())
}

/// Add an user to ignore list.
#[poise::command(slash_command, owners_only)]
async fn add<U: HasCoreEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Discord username"] discord_name: User,
) -> Result<()> {
    let env = ctx.data().core_env();
    ctx.defer().await?;
    let msg = format!("User **{}** ignored!", discord_name.name);
    env.ignore
        .add(&env.prelude, UserId(discord_name.id), discord_name.name)
        .await?;
    ctx.say(msg).await?;
    Ok(())
}

/// Remove an user from ignore list.
#[poise::command(slash_command, owners_only)]
async fn remove<U: HasCoreEnv>(
    ctx: CmdContext<'_, U>,
    #[description = "Discord username"] discord_name: User,
) -> Result<()> {
    let env = ctx.data().core_env();
    ctx.defer().await?;
    env.ignore
        .remove(&env.prelude, UserId(discord_name.id))
        .await?;
    let msg = format!("User **{}** removed from ignore list!", discord_name.name);
    ctx.say(msg).await?;
    Ok(())
}

/// List ignored users.
#[poise::command(slash_command, owners_only)]
async fn list<U: HasCoreEnv>(ctx: CmdContext<'_, U>) -> Result<()> {
    let env = ctx.data().core_env();
    let is_dm = ctx.guild_id().is_none();
    ctx.defer().await?;
    let users = env
        .ignore
        .list
        .clone()
        .iter()
        .map(|v| {
            format!(
                "- {} ({}), since <t:{}:R>",
                v.username,
                if is_dm {
                    v.id.0.mention().to_string()
                } else {
                    format!("`{}`", v.id.0.get())
                },
                v.ignored_since.timestamp(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let users = if users.is_empty() {
        "No one is being ignored!"
    } else {
        &users[..]
    };

    let msg = format!("Ignored users:\n{}", users);
    ctx.say(msg).await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct IgnoredUsers {
    list: Arc<DashMap<UserId, IgnoredUser>>,
}

impl IgnoredUsers {
    pub async fn from_db(env: &Env) -> Result<Self> {
        let list = model::IgnoredUser::get_all(&env.sql).await?;
        let mp: DashMap<_, _> = list
            .into_iter()
            .map(|v| {
                let id = (v.id as u64).into();
                (
                    id,
                    IgnoredUser {
                        id,
                        username: v.username,
                        ignored_since: v.ignored_since,
                    },
                )
            })
            .collect();
        Ok(Self { list: Arc::new(mp) })
    }

    pub fn query(&self, id: UserId) -> Option<impl std::ops::Deref<Target = IgnoredUser> + '_> {
        self.list.get(&id)
    }

    pub async fn add(&self, env: &Env, id: UserId, username: String) -> Result<()> {
        let iu = model::IgnoredUser::add(&env.sql, id.0.get() as i64, username).await?;
        self.list.insert(
            id,
            IgnoredUser {
                id,
                username: iu.username,
                ignored_since: iu.ignored_since,
            },
        );
        Ok(())
    }

    pub async fn remove(&self, env: &Env, id: UserId) -> Result<bool> {
        model::IgnoredUser::remove(&env.sql, id.0.get() as i64).await?;
        Ok(self.list.remove(&id).is_some())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct IgnoredUser {
    pub id: UserId,
    pub username: String,
    pub ignored_since: DateTime<Utc>,
}
