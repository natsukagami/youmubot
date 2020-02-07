use dotenv;
use dotenv::var;
use serenity::{
    framework::standard::{DispatchError, StandardFramework},
    model::{channel::{Message, Reaction}, gateway},
};
use youmubot_prelude::*;

struct Handler {
    hooks: Vec<fn(&mut Context, &Message) -> ()>,
}

impl Handler {
    fn new() -> Handler {
        Handler { hooks: vec![] }
    }
}

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: gateway::Ready) {
        println!("{} is connected!", ready.user.name);
    }

    fn message(&self, mut ctx: Context, message: Message) {
        println!("{:?}", message);
        self.hooks.iter().for_each(|f| f(&mut ctx, &message));
    }

    fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        ctx.data.get_cloned::<ReactionWatcher>().send(reaction);
    }
}

fn main() {
    // Setup dotenv
    if let Ok(path) = dotenv::dotenv() {
        println!("Loaded dotenv from {:?}", path);
    }

    let mut handler = Handler::new();
    // Set up hooks
    #[cfg(feature = "osu")]
    handler.hooks.push(youmubot_osu::discord::hook);

    // Sets up a client
    let mut client = {
        // Collect the token
        let token = var("TOKEN").expect("Please set TOKEN as the Discord Bot's token to be used.");
        // Attempt to connect and set up a framework
        Client::new(token, handler).expect("Cannot connect")
    };

    // Set up base framework
    let mut fw = setup_framework(&client);

    // Setup each package starting from the prelude.
    {
        let mut data = client.data.write();
        let db_path = var("DBPATH")
            .map(|v| std::path::PathBuf::from(v))
            .unwrap_or_else(|e| {
                println!("No DBPATH set up ({:?}), using `/data`", e);
                std::path::PathBuf::from("data")
            });
        youmubot_prelude::setup::setup_prelude(&db_path, &mut data, &mut fw);
        // Setup core
        #[cfg(feature = "core")]
        youmubot_core::setup(&db_path, &client, &mut data).expect("Setup db should succeed");
        // osu!
        #[cfg(feature = "osu")]
        youmubot_osu::discord::setup(&db_path, &client, &mut data).expect("osu! is initialized");
    }

    #[cfg(feature = "core")]
    println!("Core enabled.");
    #[cfg(feature = "osu")]
    println!("osu! enabled.");

    client.with_framework(fw);

    println!("Starting...");
    if let Err(v) = client.start() {
        panic!(v)
    }

    println!("Hello, world!");
}

// Sets up a framework for a client
fn setup_framework(client: &Client) -> StandardFramework {
    // Collect owners
    let owner = client
        .cache_and_http
        .http
        .get_current_application_info()
        .expect("Should be able to get app info")
        .owner;

    let fw =    StandardFramework::new()
            .configure(|c| {
                c.with_whitespace(false)
                    .prefix(&var("PREFIX").unwrap_or("y!".to_owned()))
                    .delimiters(vec![" / ", "/ ", " /", "/"])
                    .owners([owner.id].iter().cloned().collect())
            })
            .help(&youmubot_core::HELP)
            .before(|_, msg, command_name| {
                println!(
                    "Got command '{}' by user '{}'",
                    command_name, msg.author.name
                );
                true
            })
            .after(|ctx, msg, command_name, error| match error {
                Ok(()) => println!("Processed command '{}'", command_name),
                Err(why) => {
                    let reply = format!("Command '{}' returned error {:?}", command_name, why);
                    if let Err(_) = msg.reply(&ctx, &reply) {}
                    println!("{}", reply)
                }
            })
            .on_dispatch_error(|ctx, msg, error| {
                msg.reply(
                    &ctx,
                    &match error {
                        DispatchError::Ratelimited(seconds) => format!(
                            "⏳ You are being rate-limited! Try this again in **{} seconds**.",
                            seconds
                        ),
                        DispatchError::NotEnoughArguments { min, given } => format!("😕 The command needs at least **{}** arguments, I only got **{}**!\nDid you know command arguments are separated with a slash (`/`)?", min, given),
                        DispatchError::TooManyArguments { max, given } => format!("😕 I can only handle at most **{}** arguments, but I got **{}**!", max, given),
                        DispatchError::OnlyForGuilds => format!("🔇 This command cannot be used in DMs."),
                        _ => return,
                    },
                )
                .unwrap(); // Invoke
            })
            // Set a function that's called whenever an attempted command-call's
            // command could not be found.
            .unrecognised_command(|_, _, unknown_command_name| {
                println!("Could not find command named '{}'", unknown_command_name);
            })
            // Set a function that's called whenever a message is not a command.
            .normal_message(|_, _| {
                // println!("Message is not a command '{}'", message.content);
            })
            .bucket("voting", |c| {
                c.delay(120 /* 2 minutes */).time_span(120).limit(1)
            })
            .bucket("images", |c| c.time_span(60).limit(2))
            .bucket("community", |c| {
                c.delay(30).time_span(30).limit(1)
            });
    // groups here
    #[cfg(feature = "core")]
    let fw = fw
        .group(&youmubot_core::ADMIN_GROUP)
        .group(&youmubot_core::FUN_GROUP)
        .group(&youmubot_core::COMMUNITY_GROUP);
    #[cfg(feature = "osu")]
    let fw = fw.group(&youmubot_osu::discord::OSU_GROUP);
    fw
}
