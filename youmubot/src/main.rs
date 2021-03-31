use dotenv;
use dotenv::var;
use serenity::{
    client::bridge::gateway::GatewayIntents,
    framework::standard::{macros::hook, CommandResult, DispatchError, StandardFramework},
    model::{
        channel::{Channel, Message},
        gateway,
        permissions::Permissions,
    },
};
use youmubot_prelude::*;

struct Handler {
    hooks: Vec<RwLock<Box<dyn Hook>>>,
}

impl Handler {
    fn new() -> Handler {
        Handler { hooks: vec![] }
    }

    fn push_hook<T: Hook + 'static>(&mut self, f: T) {
        self.hooks.push(RwLock::new(Box::new(f)));
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: gateway::Ready) {
        // Start ReactionWatchers for community.
        #[cfg(feature = "core")]
        ctx.data
            .read()
            .await
            .get::<youmubot_core::community::ReactionWatchers>()
            .unwrap()
            .init(&ctx)
            .await;
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: Context, message: Message) {
        self.hooks
            .iter()
            .map(|hook| {
                let ctx = ctx.clone();
                let message = message.clone();
                hook.write()
                    .then(|mut h| async move { h.call(&ctx, &message).await })
            })
            .collect::<stream::FuturesUnordered<_>>()
            .for_each(|v| async move {
                if let Err(e) = v {
                    eprintln!("{}", e)
                }
            })
            .await;
    }
}

/// Returns whether the user has "MANAGE_MESSAGES" permission in the channel.
async fn is_not_channel_mod(ctx: &Context, msg: &Message) -> bool {
    match msg.channel_id.to_channel(&ctx).await {
        Ok(Channel::Guild(gc)) => gc
            .permissions_for_user(&ctx, msg.author.id)
            .await
            .map(|perms| !perms.contains(Permissions::MANAGE_MESSAGES))
            .unwrap_or(true),
        _ => true,
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    // Setup dotenv
    if let Ok(path) = dotenv::dotenv() {
        println!("Loaded dotenv from {:?}", path);
    }

    let mut handler = Handler::new();
    // Set up hooks
    #[cfg(feature = "osu")]
    handler.push_hook(youmubot_osu::discord::hook);
    #[cfg(feature = "codeforces")]
    handler.push_hook(youmubot_cf::InfoHook);

    // Collect the token
    let token = var("TOKEN").expect("Please set TOKEN as the Discord Bot's token to be used.");
    // Set up base framework
    let fw = setup_framework(&token[..]).await;

    // Sets up a client
    let mut client = {
        // Attempt to connect and set up a framework
        Client::builder(token)
            .framework(fw)
            .event_handler(handler)
            .intents(
                GatewayIntents::GUILDS
                    | GatewayIntents::GUILD_BANS
                    | GatewayIntents::GUILD_MESSAGES
                    | GatewayIntents::GUILD_MESSAGE_REACTIONS
                    | GatewayIntents::GUILD_PRESENCES
                    | GatewayIntents::GUILD_MEMBERS
                    | GatewayIntents::DIRECT_MESSAGES
                    | GatewayIntents::DIRECT_MESSAGE_REACTIONS,
            )
            .await
            .unwrap()
    };

    // Set up announcer handler
    let mut announcers = AnnouncerHandler::new(&client);

    // Setup each package starting from the prelude.
    {
        let mut data = client.data.write().await;
        let db_path = var("DBPATH")
            .map(|v| std::path::PathBuf::from(v))
            .unwrap_or_else(|e| {
                println!("No DBPATH set up ({:?}), using `/data`", e);
                std::path::PathBuf::from("/data")
            });
        let sql_path = var("SQLPATH")
            .map(|v| std::path::PathBuf::from(v))
            .unwrap_or_else(|e| {
                let res = db_path.join("youmubot.db");
                println!("No SQLPATH set up ({:?}), using `{:?}`", e, res);
                res
            });
        youmubot_prelude::setup::setup_prelude(&db_path, sql_path, &mut data).await;
        // Setup core
        #[cfg(feature = "core")]
        youmubot_core::setup(&db_path, &client, &mut data).expect("Setup db should succeed");
        // osu!
        #[cfg(feature = "osu")]
        youmubot_osu::discord::setup(&db_path, &mut data, &mut announcers)
            .expect("osu! is initialized");
        // codeforces
        #[cfg(feature = "codeforces")]
        youmubot_cf::setup(&db_path, &mut data, &mut announcers).await;
    }

    #[cfg(feature = "core")]
    println!("Core enabled.");
    #[cfg(feature = "osu")]
    println!("osu! enabled.");
    #[cfg(feature = "codeforces")]
    println!("codeforces enabled.");

    tokio::spawn(announcers.scan(std::time::Duration::from_secs(120)));

    println!("Starting...");
    if let Err(v) = client.start().await {
        panic!(v)
    }

    println!("Hello, world!");
}

// Sets up a framework for a client
async fn setup_framework(token: &str) -> StandardFramework {
    let http = serenity::http::Http::new_with_token(token);
    // Collect owners
    let owner = http
        .get_current_application_info()
        .await
        .expect("Should be able to get app info")
        .owner;

    let fw = StandardFramework::new()
        .configure(|c| {
            c.with_whitespace(false)
                .prefix(&var("PREFIX").unwrap_or("y!".to_owned()))
                .delimiters(vec![" / ", "/ ", " /", "/"])
                .owners([owner.id].iter().cloned().collect())
        })
        .help(&youmubot_core::HELP)
        .before(before_hook)
        .after(after_hook)
        .on_dispatch_error(on_dispatch_error)
        .bucket("voting", |c| {
            c.check(|ctx, msg| Box::pin(is_not_channel_mod(ctx, msg)))
                .delay(120 /* 2 minutes */)
                .time_span(120)
                .limit(1)
        })
        .await
        .bucket("images", |c| c.time_span(60).limit(2))
        .await
        .bucket("community", |c| {
            c.check(|ctx, msg| Box::pin(is_not_channel_mod(ctx, msg)))
                .delay(30)
                .time_span(30)
                .limit(1)
        })
        .await
        .group(&prelude_commands::PRELUDE_GROUP);
    // groups here
    #[cfg(feature = "core")]
    let fw = fw
        .group(&youmubot_core::ADMIN_GROUP)
        .group(&youmubot_core::FUN_GROUP)
        .group(&youmubot_core::COMMUNITY_GROUP);
    #[cfg(feature = "osu")]
    let fw = fw.group(&youmubot_osu::discord::OSU_GROUP);
    #[cfg(feature = "codeforces")]
    let fw = fw.group(&youmubot_cf::CODEFORCES_GROUP);
    fw
}

// Hooks!

#[hook]
async fn before_hook(_: &Context, msg: &Message, command_name: &str) -> bool {
    println!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );
    true
}

#[hook]
async fn after_hook(ctx: &Context, msg: &Message, command_name: &str, error: CommandResult) {
    match error {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => {
            let reply = format!("Command '{}' returned error {:?}", command_name, why);
            msg.reply(&ctx, &reply).await.ok();
            println!("{}", reply)
        }
    }
}

#[hook]
async fn on_dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    msg.reply(
        &ctx,
        &match error {
            DispatchError::Ratelimited(rl) => format!(
                "⏳ You are being rate-limited! Try this again in **{}**.",
                youmubot_prelude::Duration(rl.rate_limit),
            ),
            DispatchError::NotEnoughArguments { min, given } => {
                format!(
                    "😕 The command needs at least **{}** arguments, I only got **{}**!",
                    min, given
                ) + "\nDid you know command arguments are separated with a slash (`/`)?"
            }
            DispatchError::TooManyArguments { max, given } => format!(
                "😕 I can only handle at most **{}** arguments, but I got **{}**!",
                max, given
            ),
            DispatchError::OnlyForGuilds => format!("🔇 This command cannot be used in DMs."),
            _ => return,
        },
    )
    .await
    .ok(); // Invoke
}
