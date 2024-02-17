use crate::{AppData, MemberCache, Result};
use async_trait::async_trait;
use futures_util::{
    future::{join_all, ready, FutureExt},
    stream::{FuturesUnordered, StreamExt},
};
use serenity::{
    client::Cache,
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    http::{CacheHttp, Http},
    model::{
        channel::Message,
        id::{ChannelId, GuildId, UserId},
    },
    prelude::*,
    utils::MessageBuilder,
};
use std::{collections::HashMap, sync::Arc};
use youmubot_db::DB;

#[derive(Debug, Clone)]
pub struct CacheAndHttp(Arc<Cache>, Arc<Http>);

impl CacheAndHttp {
    pub fn from_client(client: &Client) -> Self {
        Self(client.cache.clone(), client.http.clone())
    }
}

impl CacheHttp for CacheAndHttp {
    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.0)
    }
    fn http(&self) -> &Http {
        &*self.1
    }
}

/// A list of assigned channels for an announcer.
pub(crate) type AnnouncerChannels = DB<HashMap<String, HashMap<GuildId, ChannelId>>>;

/// The Announcer trait.
///
/// Every announcer needs to implement a method to look for updates.
/// This method is called "updates", which takes:
///  - A CacheHttp implementation, for interaction with Discord itself.
///  - An AppData, which can be used for interacting with internal databases.
///  - A function "channels", which takes an UserId and returns the list of ChannelIds, which any update related to that user should be
///  sent to.
#[async_trait]
pub trait Announcer: Send {
    /// Look for updates and send them to respective channels.
    ///
    /// Errors returned from this function gets ignored and logged down.
    async fn updates(
        &mut self,
        c: CacheAndHttp,
        d: AppData,
        channels: MemberToChannels,
    ) -> Result<()>;
}

/// A simple struct that allows looking up the relevant channels to an user.
pub struct MemberToChannels(Vec<(GuildId, ChannelId)>, AppData);

impl MemberToChannels {
    /// Gets the channel list of an user related to that channel.
    pub async fn channels_of(
        &self,
        http: impl CacheHttp + Clone + Sync,
        u: impl Into<UserId>,
    ) -> Vec<ChannelId> {
        let u: UserId = u.into();
        let member_cache = self.1.read().await.get::<MemberCache>().unwrap().clone();
        self.0
            .clone()
            .into_iter()
            .map(|(guild, channel)| {
                member_cache
                    .query(http.clone(), u, guild)
                    .map(move |t| t.map(|_| channel))
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(ready)
            .collect()
            .await
    }
}

/// The announcer handler.
///
/// This struct manages the list of all Announcers, firing them in a certain interval.
pub struct AnnouncerHandler {
    cache_http: CacheAndHttp,
    data: AppData,
    announcers: HashMap<&'static str, RwLock<Box<dyn Announcer + Send + Sync>>>,
}

// Querying for the AnnouncerHandler in the internal data returns a vec of keys.
impl TypeMapKey for AnnouncerHandler {
    type Value = Vec<&'static str>;
}

/// Announcer-managing related.
impl AnnouncerHandler {
    /// Create a new instance of the handler.
    pub fn new(client: &serenity::Client) -> Self {
        Self {
            cache_http: CacheAndHttp(client.cache.clone(), client.http.clone()),
            data: client.data.clone(),
            announcers: HashMap::new(),
        }
    }

    /// Insert a new announcer into the handler.
    ///
    /// The handler must take an unique key. If a duplicate is found, this method panics.
    pub fn add(
        &mut self,
        key: &'static str,
        announcer: impl Announcer + Send + Sync + 'static,
    ) -> &mut Self {
        if self
            .announcers
            .insert(key, RwLock::new(Box::new(announcer)))
            .is_some()
        {
            panic!(
                "Announcer keys must be unique: another announcer with key `{}` was found",
                key
            )
        } else {
            self
        }
    }
}

/// Execution-related.
impl AnnouncerHandler {
    /// Collect the list of guilds and their respective channels, by the key of the announcer.
    async fn get_guilds(data: &AppData, key: &'static str) -> Result<Vec<(GuildId, ChannelId)>> {
        let data = AnnouncerChannels::open(&*data.read().await)
            .borrow()?
            .get(key)
            .map(|m| m.iter().map(|(a, b)| (*a, *b)).collect())
            .unwrap_or_else(Vec::new);
        Ok(data)
    }

    /// Run the announcing sequence on a certain announcer.
    async fn announce(
        data: AppData,
        cache_http: CacheAndHttp,
        key: &'static str,
        announcer: &'_ RwLock<Box<dyn Announcer + Send + Sync>>,
    ) -> Result<()> {
        let channels = MemberToChannels(Self::get_guilds(&data, key).await?, data.clone());
        announcer
            .write()
            .await
            .updates(cache_http, data, channels)
            .await
    }

    /// Start the AnnouncerHandler, looping forever.
    ///
    /// It will run all the announcers every *cooldown* seconds.
    pub async fn scan(self, cooldown: std::time::Duration) {
        // First we store all the keys inside the database.
        let keys = self.announcers.keys().cloned().collect::<Vec<_>>();
        self.data.write().await.insert::<Self>(keys.clone());
        join_all(self.announcers.iter().map(|(key, announcer)| {
            let data = self.data.clone();
            let cache = self.cache_http.clone();
            async move {
                loop {
                    eprintln!(" - scanning key `{}`", key);
                    match Self::announce(data.clone(), cache.clone(), key, announcer).await {
                        Err(e) => {
                            eprintln!(" - key `{}`: {:?}", *key, e)
                        }
                        Ok(()) => {
                            eprintln!(" - key `{}`: complete", *key)
                        }
                    };
                    tokio::time::sleep(cooldown).await;
                }
            }
        }))
        .await;
    }
}

/// Gets the announcer of the given guild.
pub async fn announcer_of(
    ctx: &Context,
    key: &'static str,
    guild: GuildId,
) -> Result<Option<ChannelId>> {
    Ok(AnnouncerChannels::open(&*ctx.data.read().await)
        .borrow()?
        .get(key)
        .and_then(|channels| channels.get(&guild).cloned()))
}

#[command("list")]
#[description = "List the registered announcers of this server"]
#[num_args(0)]
#[only_in(guilds)]
pub async fn list_announcers(ctx: &Context, m: &Message, _: Args) -> CommandResult {
    let guild_id = m.guild_id.unwrap();
    let data = &*ctx.data.read().await;
    let announcers = AnnouncerChannels::open(data);
    let channels = data.get::<AnnouncerHandler>().unwrap();
    let channels = channels
        .iter()
        .filter_map(|&key| {
            announcers.borrow().ok().and_then(|announcers| {
                announcers
                    .get(key)
                    .and_then(|channels| channels.get(&guild_id))
                    .map(|&ch| (key, ch))
            })
        })
        .map(|(key, ch)| format!(" - `{}`: activated on channel {}", key, ch.mention()))
        .collect::<Vec<_>>();

    m.reply(
        &ctx,
        format!(
            "Activated announcers on this server:\n{}",
            channels.join("\n")
        ),
    )
    .await?;

    Ok(())
}

#[command("register")]
#[description = "Register the current channel with an announcer"]
#[usage = "[announcer key]"]
#[required_permissions(MANAGE_CHANNELS)]
#[only_in(guilds)]
#[num_args(1)]
pub async fn register_announcer(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let key = args.single::<String>()?;
    let keys = data.get::<AnnouncerHandler>().unwrap();
    if !keys.contains(&&key[..]) {
        m.reply(
            &ctx,
            format!(
                "Key not found. Available announcer keys are: `{}`",
                keys.join(", ")
            ),
        )
        .await?;
        return Ok(());
    }
    let guild = m
        .guild_id
        .expect("Guild-only command")
        .to_partial_guild(&ctx)
        .await?;
    let channel = m.channel_id.to_channel(&ctx).await?;
    AnnouncerChannels::open(&data)
        .borrow_mut()?
        .entry(key.clone())
        .or_default()
        .insert(guild.id, m.channel_id);
    m.reply(
        &ctx,
        MessageBuilder::new()
            .push("Announcer ")
            .push_mono_safe(key)
            .push(" has been activated for server ")
            .push_bold_safe(&guild.name)
            .push(" on channel ")
            .push_bold_safe(channel.mention().to_string())
            .build(),
    )
    .await?;
    Ok(())
}

#[command("remove")]
#[description = "Remove an announcer from the server"]
#[usage = "[announcer key]"]
#[required_permissions(MANAGE_CHANNELS)]
#[only_in(guilds)]
#[num_args(1)]
pub async fn remove_announcer(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let key = args.single::<String>()?;
    let keys = data.get::<AnnouncerHandler>().unwrap();
    if !keys.contains(&key.as_str()) {
        m.reply(
            &ctx,
            format!(
                "Key not found. Available announcer keys are: `{}`",
                keys.join(", ")
            ),
        )
        .await?;
        return Ok(());
    }
    let guild = m
        .guild_id
        .expect("Guild-only command")
        .to_partial_guild(&ctx)
        .await?;
    AnnouncerChannels::open(&data)
        .borrow_mut()?
        .entry(key.clone())
        .and_modify(|m| {
            m.remove(&guild.id);
        });
    m.reply(
        &ctx,
        MessageBuilder::new()
            .push("Announcer ")
            .push_mono_safe(key)
            .push(" has been de-activated for server ")
            .push_bold_safe(&guild.name)
            .build(),
    )
    .await?;
    Ok(())
}

#[group("announcer")]
#[prefix("announcer")]
#[only_in(guilds)]
#[required_permissions(MANAGE_CHANNELS)]
#[description = "Manage the announcers in the server."]
#[commands(remove_announcer, register_announcer, list_announcers)]
pub struct AnnouncerCommands;
