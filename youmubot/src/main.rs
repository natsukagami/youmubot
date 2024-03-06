use compose_framework::ComposedFramework;
use dotenv::var;
use serenity::{
    framework::standard::{
        macros::hook, BucketBuilder, CommandError, CommandResult, Configuration, DispatchError,
        StandardFramework,
    },
    model::{
        channel::{Channel, Message},
        gateway,
        permissions::Permissions,
    },
};
use youmubot_prelude::*;

mod compose_framework;

struct Handler {
    hooks: Vec<RwLock<Box<dyn Hook>>>,
    ready_hooks: Vec<fn(&Context) -> CommandResult>,
}

impl Handler {
    fn new() -> Handler {
        Handler {
            hooks: vec![],
            ready_hooks: vec![],
        }
    }

    fn push_hook<T: Hook + 'static>(&mut self, f: T) {
        self.hooks.push(RwLock::new(Box::new(f)));
    }

    fn push_ready_hook(&mut self, f: fn(&Context) -> CommandResult) {
        self.ready_hooks.push(f);
    }
}

/// Environment to be passed into the framework
#[derive(Debug, Clone)]
struct Env {
    prelude: youmubot_prelude::Env,
    #[cfg(feature = "osu")]
    osu: youmubot_osu::discord::Env,
}

impl AsRef<youmubot_prelude::Env> for Env {
    fn as_ref(&self) -> &youmubot_prelude::Env {
        &self.prelude
    }
}

impl AsRef<youmubot_osu::discord::Env> for Env {
    fn as_ref(&self) -> &youmubot_osu::discord::Env {
        &self.osu
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

        for f in &self.ready_hooks {
            f(&ctx).pls_ok();
        }
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
            .permissions_for_user(ctx, msg.author.id)
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
    #[cfg(feature = "core")]
    handler.push_ready_hook(youmubot_core::ready_hook);
    // Set up hooks
    #[cfg(feature = "osu")]
    {
        handler.push_hook(youmubot_osu::discord::hook);
        handler.push_hook(youmubot_osu::discord::dot_osu_hook);
    }
    #[cfg(feature = "codeforces")]
    handler.push_hook(youmubot_cf::InfoHook);

    // Collect the token
    let token = var("TOKEN").expect("Please set TOKEN as the Discord Bot's token to be used.");

    // Data to be put into context
    let mut data = TypeMap::new();

    // Set up announcer handler
    let mut announcers = AnnouncerHandler::new();

    // Setup each package starting from the prelude.
    let env = {
        let db_path = var("DBPATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|e| {
                println!("No DBPATH set up ({:?}), using `/data`", e);
                std::path::PathBuf::from("/data")
            });
        let sql_path = var("SQLPATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|e| {
                let res = db_path.join("youmubot.db");
                println!("No SQLPATH set up ({:?}), using `{:?}`", e, res);
                res
            });
        let prelude = youmubot_prelude::setup::setup_prelude(&db_path, sql_path, &mut data).await;
        // Setup core
        #[cfg(feature = "core")]
        youmubot_core::setup(&db_path, &mut data).expect("Setup db should succeed");
        // osu!
        #[cfg(feature = "osu")]
        let osu = youmubot_osu::discord::setup(&mut data, prelude.clone(), &mut announcers)
            .await
            .expect("osu! is initialized");
        // codeforces
        #[cfg(feature = "codeforces")]
        youmubot_cf::setup(&db_path, &mut data, &mut announcers).await;

        Env {
            prelude,
            #[cfg(feature = "osu")]
            osu,
        }
    };

    #[cfg(feature = "core")]
    println!("Core enabled.");
    #[cfg(feature = "osu")]
    println!("osu! enabled.");
    #[cfg(feature = "codeforces")]
    println!("codeforces enabled.");

    // Set up base framework
    let fw = setup_framework(&token[..]).await;

    // Poise for application commands
    let poise_fw = poise::Framework::builder()
        .setup(|_, _, _| Box::pin(async { Ok(env) as Result<_, CommandError> }))
        .options(poise::FrameworkOptions {
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: None,
                mention_as_prefix: true,
                execute_untracked_edits: true,
                execute_self_messages: false,
                ignore_thread_creation: true,
                case_insensitive_commands: true,
                ..Default::default()
            },
            on_error: |err| {
                Box::pin(async move {
                    if let poise::FrameworkError::Command { error, ctx, .. } = err {
                        let reply = format!(
                            "Command '{}' returned error {:?}",
                            ctx.invoked_command_name(),
                            error
                        );
                        ctx.reply(&reply).await.pls_ok();
                        println!("{}", reply)
                    } else {
                        eprintln!("Poise error: {:?}", err)
                    }
                })
            },
            commands: vec![poise_register(), youmubot_osu::discord::app_commands::osu()],
            ..Default::default()
        })
        .build();

    let composed = ComposedFramework::new(vec![Box::new(fw), Box::new(poise_fw)]);

    // Sets up a client
    let mut client = {
        // Attempt to connect and set up a framework
        let intents = GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MODERATION
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::GUILD_MESSAGE_REACTIONS
            | GatewayIntents::GUILD_PRESENCES
            | GatewayIntents::GUILD_MEMBERS
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::DIRECT_MESSAGE_REACTIONS;
        Client::builder(token, intents)
            .type_map(data)
            .framework(composed)
            .event_handler(handler)
            .await
            .unwrap()
    };

    let announcers = announcers.run(&client);
    tokio::spawn(announcers.scan(std::time::Duration::from_secs(300)));

    println!("Starting...");
    if let Err(v) = client.start().await {
        panic!("{}", v)
    }
}

// Sets up a framework for a client
async fn setup_framework(token: &str) -> StandardFramework {
    let http = serenity::http::Http::new(token);
    // Collect owners
    let owner = http
        .get_current_application_info()
        .await
        .expect("Should be able to get app info")
        .owner
        .unwrap();

    let fw = StandardFramework::new()
        .help(&youmubot_core::HELP)
        .before(before_hook)
        .after(after_hook)
        .on_dispatch_error(on_dispatch_error)
        .bucket("voting", {
            BucketBuilder::new_channel()
                .check(|ctx, msg| Box::pin(is_not_channel_mod(ctx, msg)))
                .delay(120 /* 2 minutes */)
                .time_span(120)
                .limit(1)
        })
        .await
        .bucket(
            "images",
            BucketBuilder::new_channel().time_span(60).limit(2),
        )
        .await
        .bucket(
            "community",
            BucketBuilder::new_guild()
                .check(|ctx, msg| Box::pin(is_not_channel_mod(ctx, msg)))
                .delay(30)
                .time_span(30)
                .limit(1),
        )
        .await
        .group(&prelude_commands::PRELUDE_GROUP);
    fw.configure(
        Configuration::new()
            .with_whitespace(false)
            .prefixes(
                var("PREFIX")
                    .map(|v| v.split(',').map(|v| v.trim().to_owned()).collect())
                    .unwrap_or_else(|_| vec!["y!".to_owned(), "y2!".to_owned()]),
            )
            .delimiters(vec![" / ", "/ ", " /", "/"])
            .owners([owner.id].iter().cloned().collect()),
    );
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

// Poise command to register
#[poise::command(
    prefix_command,
    rename = "register",
    required_permissions = "MANAGE_GUILD"
)]
async fn poise_register(ctx: CmdContext<'_, Env>) -> CommandResult {
    // TODO: make this work for guild owners too
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
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
async fn on_dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _cmd_name: &str) {
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
            DispatchError::OnlyForGuilds => "🔇 This command cannot be used in DMs.".to_owned(),
            _ => return,
        },
    )
    .await
    .ok(); // Invoke
}
