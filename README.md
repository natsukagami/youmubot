# youmubot 

![Deploy](https://github.com/natsukagami/youmubot/workflows/Deploy/badge.svg)
![Build and Test](https://github.com/natsukagami/youmubot/workflows/Build%20and%20Test/badge.svg)

A Discord bot made specifically for server "Dự tuyển Tổng Hợp". Written in Rust.

All PRs welcome.

## Project structure

- `youmubot`: The main command. Collect configurations and dispatch commands.
- `youmubot-prelude`: Base structures and handy functions for command parsing / service handling.
- `youmubot-db`: Base database structures.
- `youmubot-core`: Core commands: admin, fun, community
- `youmubot-osu`: osu!-related commands.

## Working with `sqlx`

### Regenerate compiler information

The commands expect the cwd to be at the project base directory.

Manually run migrations with
```
sqlx migrate run --database-url "sqlite:./youmubot.db" --source ./youmubot-db-sql/migrations
```

Update compiler information with
```bash
cargo sqlx prepare --database-url "sqlite:./youmubot.db" --workspace
```

## License

Basically MIT.
