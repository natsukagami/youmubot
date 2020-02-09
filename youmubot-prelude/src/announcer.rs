use crate::AppData;
use rayon::prelude::*;
use serenity::{
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    http::CacheHttp,
    model::{
        channel::Message,
        id::{ChannelId, GuildId, UserId},
    },
    prelude::*,
    CacheAndHttp,
};
use std::{
    collections::HashMap,
    sync::Arc,
    thread::{spawn, JoinHandle},
};
use youmubot_db::DB;

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
pub trait Announcer: Send {
    /// Look for updates and send them to respective channels.
    ///
    /// Errors returned from this function gets ignored and logged down.
    fn updates(
        &mut self,
        c: Arc<CacheAndHttp>,
        d: AppData,
        channels: MemberToChannels,
    ) -> CommandResult;
}

impl<T> Announcer for T
where
    T: FnMut(Arc<CacheAndHttp>, AppData, MemberToChannels) -> CommandResult + Send,
{
    fn updates(
        &mut self,
        c: Arc<CacheAndHttp>,
        d: AppData,
        channels: MemberToChannels,
    ) -> CommandResult {
        self(c, d, channels)
    }
}

/// A simple struct that allows looking up the relevant channels to an user.
pub struct MemberToChannels(Vec<(GuildId, ChannelId)>);

impl MemberToChannels {
    /// Gets the channel list of an user related to that channel.
    pub fn channels_of(
        &self,
        http: impl CacheHttp + Clone + Sync,
        u: impl Into<UserId>,
    ) -> Vec<ChannelId> {
        let u = u.into();
        self.0
            .par_iter()
            .filter_map(|(guild, channel)| {
                guild.member(http.clone(), u).ok().map(|_| channel.clone())
            })
            .collect::<Vec<_>>()
    }
}

/// The announcer handler.
///
/// This struct manages the list of all Announcers, firing them in a certain interval.
pub struct AnnouncerHandler {
    cache_http: Arc<CacheAndHttp>,
    data: AppData,
    announcers: HashMap<&'static str, Box<dyn Announcer>>,
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
            cache_http: client.cache_and_http.clone(),
            data: client.data.clone(),
            announcers: HashMap::new(),
        }
    }

    /// Insert a new announcer into the handler.
    ///
    /// The handler must take an unique key. If a duplicate is found, this method panics.
    pub fn add(&mut self, key: &'static str, announcer: impl Announcer + 'static) -> &mut Self {
        if let Some(_) = self.announcers.insert(key, Box::new(announcer)) {
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
    fn get_guilds(&self, key: &'static str) -> Result<Vec<(GuildId, ChannelId)>, Error> {
        let d = &self.data;
        let data = AnnouncerChannels::open(&*d.read())
            .borrow()?
            .get(key)
            .map(|m| m.iter().map(|(a, b)| (*a, *b)).collect())
            .unwrap_or_else(|| vec![]);
        Ok(data)
    }

    /// Run the announcing sequence on a certain announcer.
    fn announce(&mut self, key: &'static str) -> CommandResult {
        let guilds: Vec<_> = self.get_guilds(key)?;
        let channels = MemberToChannels(guilds);
        let cache_http = self.cache_http.clone();
        let data = self.data.clone();
        let announcer = self
            .announcers
            .get_mut(&key)
            .expect("Key is from announcers");
        announcer.updates(cache_http, data, channels)?;
        Ok(())
    }

    /// Start the AnnouncerHandler, moving it into another thread.
    ///
    /// It will run all the announcers in sequence every *cooldown* seconds.
    pub fn scan(mut self, cooldown: std::time::Duration) -> JoinHandle<()> {
        // First we store all the keys inside the database.
        let keys = self.announcers.keys().cloned().collect::<Vec<_>>();
        self.data.write().insert::<Self>(keys.clone());
        spawn(move || loop {
            for key in &keys {
                if let Err(e) = self.announce(key) {
                    dbg!(e);
                }
            }
            std::thread::sleep(cooldown);
        })
    }
}

#[command("register")]
#[description = "Register the current channel with an announcer"]
#[usage = "[announcer key]"]
pub fn register_announcer(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    unimplemented!()
}
