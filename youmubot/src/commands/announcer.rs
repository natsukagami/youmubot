use serenity::{
    builder::CreateMessage,
    framework::standard::{CommandError as Error, CommandResult},
    http::{CacheHttp, Http},
    model::id::{ChannelId, GuildId, UserId},
};
use std::{
    collections::HashSet,
    thread::{spawn, JoinHandle},
};

pub trait Announcer {
    type MessageSender: for<'a, 'r> Fn(&'r mut CreateMessage<'a>) -> &'r mut CreateMessage<'a>;

    fn get_guilds(c: impl AsRef<Http>) -> Result<Vec<(GuildId, ChannelId)>, Error>;
    fn fetch_messages(c: impl AsRef<Http>) -> Result<Vec<(UserId, Self::MessageSender)>, Error>;

    fn announce(c: impl AsRef<Http>) -> CommandResult {
        let guilds: Vec<_> = Self::get_guilds(c.as_ref())?;
        let member_sets = {
            let mut v = Vec::with_capacity(guilds.len());
            for (guild, channel) in guilds.into_iter() {
                let mut s = HashSet::new();
                for user in guild.members_iter(c.as_ref()) {
                    s.insert(user?.user_id());
                }
                v.push((s, channel))
            }
            v
        };
        for (user_id, f) in Self::fetch_messages(c.as_ref())?.into_iter() {
            for (members, channel) in member_sets.iter() {
                if members.contains(&user_id) {
                    if let Err(e) = channel.send_message(c.as_ref(), &f) {
                        dbg!((user_id, channel, e));
                    }
                }
            }
        }
        Ok(())
    }

    fn scan(c: impl CacheHttp + 'static + Send, cooldown: std::time::Duration) -> JoinHandle<()> {
        spawn(move || loop {
            if let Err(e) = Self::announce(c.http()) {
                dbg!(e);
            }
            std::thread::sleep(cooldown);
        })
    }
}
