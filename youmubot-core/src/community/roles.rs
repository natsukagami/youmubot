use serenity::{
    builder::EditMessage,
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Message, ReactionType},
        guild::Role,
        id::RoleId,
    },
    utils::MessageBuilder,
};

pub use reaction_watcher::Watchers as ReactionWatchers;
use youmubot_prelude::table_format::Align::Right;
use youmubot_prelude::table_format::{table_formatting, Align};
use youmubot_prelude::*;

use crate::db::Roles as DB;

#[command("listroles")]
#[description = "List all available roles in the server."]
#[num_args(0)]
#[only_in(guilds)]
async fn list(ctx: &Context, m: &Message, _: Args) -> CommandResult {
    let guild_id = m.guild_id.unwrap(); // only_in(guilds)
    let data = ctx.data.read().await;
    let db = DB::open(&data);
    let roles = db
        .borrow()?
        .get(&guild_id)
        .filter(|v| !v.roles.is_empty())
        .cloned();
    match roles {
        None => {
            m.reply(&ctx, "No roles available for assigning.").await?;
        }
        Some(v) => {
            let roles = guild_id.to_partial_guild(&ctx).await?.roles;
            let roles: Vec<_> = v
                .roles
                .into_iter()
                .filter_map(|(_, role)| roles.get(&role.id).cloned().map(|r| (r, role.description)))
                .collect();
            const ROLES_PER_PAGE: usize = 8;
            let pages = (roles.len() + ROLES_PER_PAGE - 1) / ROLES_PER_PAGE;

            paginate_reply_fn(
                |page, ctx, msg| {
                    let roles = roles.clone();
                    Box::pin(async move {
                        let page = page as usize;
                        let start = page * ROLES_PER_PAGE;
                        let end = roles.len().min(start + ROLES_PER_PAGE);
                        if end <= start {
                            return Ok(false);
                        }

                        let roles = &roles[start..end];

                        const ROLE_HEADERS: [&'static str; 3] = ["Name", "ID", "Description"];
                        const ROLE_ALIGNS: [Align; 3] = [Right, Right, Right];

                        let roles_arr = roles
                            .iter()
                            .map(|(role, description)| {
                                [
                                    role.name.clone(),
                                    format!("{}", role.id),
                                    description.clone(),
                                ]
                            })
                            .collect::<Vec<_>>();

                        let roles_table = table_formatting(&ROLE_HEADERS, &ROLE_ALIGNS, roles_arr);

                        let content = MessageBuilder::new()
                            .push_line(roles_table)
                            .push_line(format!("Page **{}/{}**", page + 1, pages))
                            .build();

                        msg.edit(ctx, EditMessage::new().content(content)).await?;
                        Ok(true)
                    })
                },
                ctx,
                m,
                std::time::Duration::from_secs(60 * 10),
            )
            .await?;
        }
    };
    Ok(())
}

// async fn list_pager(

#[command("role")]
#[description = "Toggle a role by its name or ID."]
#[example = "\"IELTS / TOEFL\""]
#[num_args(1)]
#[only_in(guilds)]
async fn toggle(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let guild_id = m.guild_id.unwrap();
    let guild = guild_id.to_partial_guild(&ctx).await?;
    let role = role_from_string(&role, &guild.roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists").await?;
        }
        Some(role)
            if !DB::open(&*ctx.data.read().await)
                .borrow()?
                .get(&guild_id)
                .map(|g| g.roles.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role is not self-assignable. Check the `listroles` command to see which role can be assigned.").await?;
        }
        Some(role) => {
            let member = guild.member(&ctx, m.author.id).await.unwrap();
            if member.roles.contains(&role.id) {
                member.remove_role(&ctx, &role).await?;
                m.reply(&ctx, format!("Role `{}` has been removed.", role.name))
                    .await?;
            } else {
                member.add_role(&ctx, &role).await?;
                m.reply(&ctx, format!("Role `{}` has been assigned.", role.name))
                    .await?;
            }
        }
    };
    Ok(())
}

#[command("addrole")]
#[description = "Add a role as the assignable role. Overrides the old entry."]
#[usage = "{role-name-or-id} / {description} / [representing emoji = none]"]
#[example = "hd820 / Headphones role / 🎧"]
#[min_args(2)]
#[max_args(3)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn add(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let data = ctx.data.read().await;
    let description = args.single_quoted::<String>()?;
    let reaction = match args.single::<ReactionType>() {
        Ok(v) => match &v {
            ReactionType::Custom { id, .. } => {
                // Verify that the reaction type is from the server.
                if m.guild_id.unwrap().emoji(&ctx, *id).await.is_err() {
                    m.reply(&ctx, "Emote cannot be used as I cannot send this back.")
                        .await?;
                    return Ok(());
                }
                Some(v)
            }
            _ => Some(v),
        },
        _ => None,
    };
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx).await?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists").await?;
        }
        Some(role) => {
            DB::open(&data)
                .borrow_mut()?
                .entry(guild_id)
                .or_default()
                .roles
                .insert(
                    role.id,
                    crate::db::Role {
                        id: role.id,
                        description,
                        reaction,
                    },
                );
            m.react(&ctx, '👌').await?;
        }
    };
    Ok(())
}

#[command("removerole")]
#[description = "Remove a role from the assignable roles list."]
#[usage = "{role-name-or-id}"]
#[example = "hd820"]
#[num_args(1)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn remove(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let data = ctx.data.read().await;
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx).await?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists").await?;
        }
        Some(role)
            if !DB::open(&data)
                .borrow()?
                .get(&guild_id)
                .map(|g| g.roles.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role does not exist in the assignable list.")
                .await?;
        }
        Some(role) => {
            DB::open(&data)
                .borrow_mut()?
                .entry(guild_id)
                .or_default()
                .roles
                .remove(&role.id);
            m.react(&ctx, '👌').await?;
        }
    };
    Ok(())
}

/// Parse a string as a role.
fn role_from_string(role: &str, roles: &std::collections::HashMap<RoleId, Role>) -> Option<Role> {
    match role.parse::<u64>() {
        Ok(id) if roles.contains_key(&RoleId::new(id)) => roles.get(&RoleId::new(id)).cloned(),
        _ => roles
            .iter()
            .find_map(|(_, r)| if r.name == role { Some(r) } else { None })
            .cloned(),
    }
}

#[command("rolemessage")]
#[description = "Create a message that handles roles in a list. All roles in the list must already be inside the set. Empty = all assignable roles."]
#[usage = "{title}/[role]/[role]/..."]
#[example = "Game Roles/Genshin/osu!"]
#[min_args(1)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn rolemessage(ctx: &Context, m: &Message, args: Args) -> CommandResult {
    let (title, roles) = match parse(ctx, m, args).await? {
        Some(v) => v,
        None => return Ok(()),
    };
    let data = ctx.data.read().await;
    let guild_id = m.guild_id.unwrap();
    data.get::<ReactionWatchers>()
        .unwrap()
        .add(ctx.clone(), guild_id, m.channel_id, title, roles)
        .await?;
    Ok(())
}

async fn parse(
    ctx: &Context,
    m: &Message,
    mut args: Args,
) -> Result<Option<(String, Vec<(crate::db::Role, Role, ReactionType)>)>> {
    let title = args.single_quoted::<String>().unwrap();
    let data = ctx.data.read().await;
    let guild_id = m.guild_id.unwrap();
    let assignables = DB::open(&data)
        .borrow()?
        .get(&guild_id)
        .filter(|v| !v.roles.is_empty())
        .map(|r| r.roles.clone())
        .unwrap_or_default();
    let mut rolenames = args
        .iter::<String>()
        .quoted()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let rolelist = guild_id.to_partial_guild(&ctx).await?.roles;
    let mut roles = Vec::new();
    if rolenames.is_empty() {
        rolenames = assignables.keys().map(|v| v.to_string()).collect();
    }
    for rolename in rolenames {
        let role = match role_from_string(rolename.as_str(), &rolelist) {
            Some(role) => role,
            None => {
                m.reply(&ctx, format!("Role `{}` not found", rolename))
                    .await?;
                return Ok(None);
            }
        };
        let role = match assignables.get(&role.id) {
            Some(r) => match &r.reaction {
                Some(emote) => (r.clone(), role.clone(), emote.clone()),
                None => {
                    m.reply(
                        &ctx,
                        format!("Role `{}` does not have a assignable emote.", rolename),
                    )
                    .await?;
                    return Ok(None);
                }
            },
            None => {
                m.reply(&ctx, format!("Role `{}` is not assignable.", rolename))
                    .await?;
                return Ok(None);
            }
        };
        roles.push(role);
    }
    Ok(Some((title, roles)))
}

#[command("updaterolemessage")]
#[description = "Update the role message to use the new list and title. All roles in the list must already be inside the set. Empty = all assignable roles."]
#[usage = "{title}/[role]/[role]/..."]
#[example = "Game Roles/Genshin/osu!"]
#[min_args(1)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn updaterolemessage(ctx: &Context, m: &Message, args: Args) -> CommandResult {
    let (title, roles) = match parse(ctx, m, args).await? {
        Some(v) => v,
        None => return Ok(()),
    };
    let data = ctx.data.read().await;
    let guild_id = m.guild_id.unwrap();

    let mut message = match &m.referenced_message {
        Some(m) => m,
        None => {
            m.reply(&ctx, "No replied message found.").await?;
            return Ok(());
        }
    }
    .clone();

    if data
        .get::<ReactionWatchers>()
        .unwrap()
        .remove(ctx, guild_id, message.id)
        .await?
    {
        data.get::<ReactionWatchers>()
            .unwrap()
            .setup(&mut message, ctx.clone(), guild_id, title, roles)
            .await?;
    } else {
        m.reply(&ctx, "Message does not come with a reaction handler")
            .await
            .ok();
    }

    Ok(())
}

#[command("rmrolemessage")]
#[description = "Delete a role message handler."]
#[usage = "(reply to the message to delete)"]
#[num_args(0)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn rmrolemessage(ctx: &Context, m: &Message, _args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let guild_id = m.guild_id.unwrap();

    let message = match &m.referenced_message {
        Some(m) => m,
        None => {
            m.reply(&ctx, "No replied message found.").await?;
            return Ok(());
        }
    };

    if !data
        .get::<ReactionWatchers>()
        .unwrap()
        .remove(ctx, guild_id, message.id)
        .await?
    {
        m.reply(&ctx, "Message does not come with a reaction handler")
            .await
            .ok();
    }

    Ok(())
}

mod reaction_watcher {
    use dashmap::DashMap;
    use flume::{Receiver, Sender};
    use serenity::{
        all::Reaction,
        builder::{CreateMessage, EditMessage},
        model::{
            channel::{Message, ReactionType},
            guild::Role as DiscordRole,
            id::{ChannelId, GuildId, MessageId},
        },
    };

    use youmubot_prelude::*;

    use crate::db::{Role, RoleMessage, Roles};

    /// A set of watchers.
    #[derive(Debug)]
    pub struct Watchers {
        watchers: DashMap<MessageId, Watcher>,

        init: Mutex<Vec<(GuildId, MessageId, RoleMessage)>>,
    }

    impl Watchers {
        pub fn new(data: &TypeMap) -> Result<Self> {
            let init = Roles::open(data)
                .borrow()?
                .iter()
                .flat_map(|(&guild, rs)| {
                    rs.reaction_messages
                        .iter()
                        .map(move |(m, r)| (guild, *m, r.clone()))
                })
                .collect();
            Ok(Self {
                init: Mutex::new(init),
                watchers: DashMap::new(),
            })
        }
        pub async fn init(&self, ctx: &Context) {
            let mut init = self.init.lock().await;
            for (msg, watcher) in init
                .drain(..)
                .map(|(guild, msg, rm)| (msg, Watcher::spawn(ctx.clone(), guild, rm.id)))
            {
                self.watchers.insert(msg, watcher);
            }
        }
        pub async fn add(
            &self,
            ctx: Context,
            guild: GuildId,
            channel: ChannelId,
            title: String,
            roles: Vec<(Role, DiscordRole, ReactionType)>,
        ) -> Result<()> {
            let mut msg = channel
                .send_message(
                    &ctx,
                    CreateMessage::new().content("Youmu is setting up the message..."),
                )
                .await?;
            self.setup(&mut msg, ctx, guild, title, roles).await
        }
        pub async fn setup(
            &self,
            msg: &mut Message,
            ctx: Context,
            guild: GuildId,
            title: String,
            roles: Vec<(Role, DiscordRole, ReactionType)>,
        ) -> Result<()> {
            // Send a message
            msg.edit(
                &ctx,
                EditMessage::new().content({
                    let mut builder = serenity::utils::MessageBuilder::new();
                    builder
                        .push_bold("Role Menu:")
                        .push(" ")
                        .push_bold_line_safe(&title)
                        .push_line("React to give yourself a role.")
                        .push_line("");
                    for (role, discord_role, emoji) in &roles {
                        builder
                            .push(emoji.to_string())
                            .push(" ")
                            .push_bold_safe(&discord_role.name)
                            .push(": ")
                            .push_line_safe(&role.description)
                            .push_line("");
                    }
                    builder.build()
                }),
            )
            .await?;
            // Do reactions
            for (_, _, emoji) in &roles {
                msg.react(&ctx, emoji.clone()).await.ok();
            }
            // Store the message into the list.
            {
                let data = ctx.data.read().await;
                Roles::open(&data)
                    .borrow_mut()?
                    .entry(guild)
                    .or_default()
                    .reaction_messages
                    .insert(
                        msg.id,
                        RoleMessage {
                            id: msg.id,
                            title,
                            roles: roles.into_iter().map(|(a, _, b)| (a, b)).collect(),
                        },
                    );
            }
            // Spawn the handler
            self.watchers
                .insert(msg.id, Watcher::spawn(ctx, guild, msg.id));
            Ok(())
        }

        pub async fn remove(
            &self,
            ctx: &Context,
            guild: GuildId,
            message: MessageId,
        ) -> Result<bool> {
            let data = ctx.data.read().await;
            Roles::open(&data)
                .borrow_mut()?
                .entry(guild)
                .or_default()
                .reaction_messages
                .remove(&message);
            Ok(self.watchers.remove(&message).is_some())
        }
    }

    impl TypeMapKey for Watchers {
        type Value = Watchers;
    }

    /// A reaction watcher structure. Contains a cancel signaler that cancels the watcher upon Drop.
    #[derive(Debug)]
    struct Watcher {
        cancel: Sender<()>,
    }

    impl Watcher {
        pub fn spawn(ctx: Context, guild: GuildId, message: MessageId) -> Self {
            let (send, recv) = flume::bounded(0);
            tokio::spawn(Self::handle(ctx, recv, guild, message));
            Watcher { cancel: send }
        }

        async fn handle(ctx: Context, recv: Receiver<()>, guild: GuildId, message: MessageId) {
            let mut recv = recv.into_recv_async();
            let mut collect = serenity::collector::collect(&ctx.shard, move |event| {
                match event {
                    serenity::all::Event::ReactionAdd(r) => Some((r.reaction.clone(), true)),
                    serenity::all::Event::ReactionRemove(r) => Some((r.reaction.clone(), false)),
                    _ => None,
                }
                .filter(|(r, _)| r.message_id == message)
            });
            // serenity::collector::CollectReaction::new(&ctx)
            //     .message_id(message)
            //     .removed(true)
            loop {
                let (reaction, is_add) = match future::select(recv, collect.next()).await {
                    future::Either::Left(_) => break,
                    future::Either::Right((r, new_recv)) => {
                        recv = new_recv;
                        match r {
                            Some(r) => r,
                            None => continue,
                        }
                    }
                };
                eprintln!("{:?} {}", reaction, is_add);
                if let Err(e) = Self::handle_reaction(&ctx, guild, message, &reaction, is_add).await
                {
                    eprintln!("Handling {:?}: {}", reaction, e);
                    break;
                }
            }
        }

        async fn handle_reaction(
            ctx: &Context,
            guild: GuildId,
            message: MessageId,
            reaction: &Reaction,
            is_add: bool,
        ) -> Result<()> {
            let data = ctx.data.read().await;
            // Collect user
            let user_id = match reaction.user_id {
                Some(id) => id,
                None => return Ok(()),
            };
            let member = match guild.member(ctx, user_id).await.ok() {
                Some(m) => m,
                None => return Ok(()),
            };
            if member.user.bot {
                return Ok(());
            }
            // Get the role list.
            let role = Roles::open(&data)
                .borrow()?
                .get(&guild)
                .ok_or_else(|| Error::msg("guild no longer has role list"))?
                .reaction_messages
                .get(&message)
                .map(|msg| &msg.roles[..])
                .ok_or_else(|| Error::msg("message is no longer a role list handler"))?
                .iter()
                .find_map(|(role, role_reaction)| {
                    if &reaction.emoji == role_reaction {
                        Some(role.id)
                    } else {
                        None
                    }
                });
            let role = match role {
                Some(id) => id,
                None => return Ok(()),
            };

            if is_add {
                member.add_role(&ctx, role).await.pls_ok();
            } else {
                member.remove_role(&ctx, role).await.pls_ok();
            };
            Ok(())
        }
    }

    impl Drop for Watcher {
        fn drop(&mut self) {
            self.cancel.send(()).unwrap()
        }
    }
}
