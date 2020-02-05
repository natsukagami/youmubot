use dotenv;
use dotenv::var;
use serenity::{
    framework::standard::{DispatchError, StandardFramework},
    model::{channel::Message, gateway},
};
use youmubot_osu::Client as OsuApiClient;
use youmubot_prelude::*;

mod commands;
mod db;

use commands::osu::OsuAnnouncer;

const MESSAGE_HOOKS: [fn(&mut Context, &Message) -> (); 1] = [commands::osu::hook];

struct Handler;

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: gateway::Ready) {
        println!("{} is connected!", ready.user.name);
    }

    fn message(&self, mut ctx: Context, message: Message) {
        MESSAGE_HOOKS.iter().for_each(|f| f(&mut ctx, &message));
    }
}

fn main() {
    // Setup dotenv
    if let Ok(path) = dotenv::dotenv() {
        println!("Loaded dotenv from {:?}", path);
    }

    // Sets up a client
    let mut client = {
        // Collect the token
        let token = var("TOKEN").expect("Please set TOKEN as the Discord Bot's token to be used.");
        // Attempt to connect and set up a framework
        Client::new(token, Handler).expect("Cannot connect")
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
    }

    // Setup initial data
    db::setup_db(&mut client).expect("Setup db should succeed");
    // Setup shared instances of things
    {
        let mut data = client.data.write();
        let http_client = data.get_cloned::<HTTPClient>();
        data.insert::<OsuClient>(OsuApiClient::new(
            http_client.clone(),
            var("OSU_API_KEY").expect("Please set OSU_API_KEY as osu! api key."),
        ));
    }

    // Create handler threads
    std::thread::spawn(commands::admin::watch_soft_bans(&mut client));

    // Announcers
    OsuAnnouncer::scan(&client, std::time::Duration::from_secs(300));

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

    StandardFramework::new()
            .configure(|c| {
                c.with_whitespace(false)
                    .prefix("y!")
                    .delimiters(vec![" / ", "/ ", " /", "/"])
                    .owners([owner.id].iter().cloned().collect())
            })
            .help(&commands::HELP)
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
            })
            // groups here
            .group(&commands::ADMIN_GROUP)
            .group(&commands::FUN_GROUP)
            .group(&commands::COMMUNITY_GROUP)
            .group(&commands::OSU_GROUP)
}
