use crate::db::{AnnouncerChannels, DBWriteGuard};
use serenity::{
    framework::standard::{CommandError as Error, CommandResult},
    http::{CacheHttp, Http},
    model::id::{ChannelId, GuildId, UserId},
    prelude::ShareMap,
};
use std::{
    collections::HashSet,
    thread::{spawn, JoinHandle},
};

pub trait Announcer {
    fn announcer_key() -> &'static str;
    fn send_messages(
        c: &Http,
        d: &mut ShareMap,
        channels: impl Fn(UserId) -> Vec<ChannelId> + Sync,
    ) -> CommandResult;

    fn set_channel(d: &mut ShareMap, guild: GuildId, channel: ChannelId) -> CommandResult {
        let mut data: DBWriteGuard<_> = d
            .get_mut::<AnnouncerChannels>()
            .expect("DB initialized")
            .into();
        let mut data = data.borrow_mut()?;
        data.entry(Self::announcer_key().to_owned())
            .or_default()
            .insert(guild, channel);
        Ok(())
    }

    fn get_guilds(d: &mut ShareMap) -> Result<Vec<(GuildId, ChannelId)>, Error> {
        let data = d
            .get::<AnnouncerChannels>()
            .expect("DB initialized")
            .read(|v| {
                v.get(Self::announcer_key())
                    .map(|m| m.iter().map(|(a, b)| (*a, *b)).collect())
                    .unwrap_or_else(|| vec![])
            })?;
        Ok(data)
    }

    fn announce(c: &Http, d: &mut ShareMap) -> CommandResult {
        let guilds: Vec<_> = Self::get_guilds(d)?;
        let member_sets = {
            let mut v = Vec::with_capacity(guilds.len());
            for (guild, channel) in guilds.into_iter() {
                let mut s = HashSet::new();
                for user in guild
                    .members_iter(c.as_ref())
                    .take_while(|u| u.is_ok())
                    .filter_map(|u| u.ok())
                {
                    s.insert(user.user_id());
                }
                v.push((s, channel))
            }
            v
        };
        Self::send_messages(c.as_ref(), d, |user_id| {
            let mut v = Vec::new();
            for (members, channel) in member_sets.iter() {
                if members.contains(&user_id) {
                    v.push(*channel);
                }
            }
            v
        })?;
        Ok(())
    }

    fn scan(client: &serenity::Client, cooldown: std::time::Duration) -> JoinHandle<()> {
        let c = client.cache_and_http.clone();
        let data = client.data.clone();
        spawn(move || loop {
            if let Err(e) = Self::announce(c.http(), &mut *data.write()) {
                dbg!(e);
            }
            std::thread::sleep(cooldown);
        })
    }
}
