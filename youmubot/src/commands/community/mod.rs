use serenity::framework::standard::macros::group;

mod votes;

use votes::VOTE_COMMAND;

group!({
    name: "community",
    options: {
        description: "Community related commands. Usually comes with some sort of delays, since it involves pinging",
    },
    commands: [vote],
});
