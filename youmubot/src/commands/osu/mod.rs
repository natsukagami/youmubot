use serenity::framework::standard::macros::group;

mod hook;

pub use hook::hook;

group!({
    name: "osu",
    options: {
        prefix: "osu",
        description: "osu! related commands.",
    },
    commands: [],
});
