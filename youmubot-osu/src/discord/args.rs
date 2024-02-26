use serenity::all::Message;
use youmubot_prelude::*;

// One of the interaction sources.
pub enum InteractionSrc<'a, 'c: 'a, T, E> {
    Serenity(&'a Message),
    Poise(&'a poise::Context<'c, T, E>),
}

impl<'a, 'c, T, E> InteractionSrc<'a, 'c, T, E> {
    pub async fn reply(&self, ctx: &Context, msg: impl Into<String>) -> Result<Message> {
        Ok(match self {
            InteractionSrc::Serenity(m) => m.reply(ctx, msg).await?,
            InteractionSrc::Poise(ctx) => ctx.reply(msg).await?.message().await?.into_owned(),
        })
    }
}

impl<'a, 'c, T, E> From<&'a Message> for InteractionSrc<'a, 'c, T, E> {
    fn from(value: &'a Message) -> Self {
        Self::Serenity(value)
    }
}

impl<'a, 'c, T, E> From<&'a poise::Context<'c, T, E>> for InteractionSrc<'a, 'c, T, E> {
    fn from(value: &'a poise::Context<'c, T, E>) -> Self {
        Self::Poise(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, poise::ChoiceParameter, Default)]
pub enum ScoreDisplay {
    #[name = "table"]
    #[default]
    Table,
    #[name = "grid"]
    Grid,
}

impl std::str::FromStr for ScoreDisplay {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "--table" => Ok(Self::Table),
            "--grid" => Ok(Self::Grid),
            _ => Err(Error::unknown(s)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unknown value: {0}")]
    UnknownValue(String),
    #[error("parse error: {0}")]
    Custom(String),
}

impl Error {
    fn unknown(s: impl AsRef<str>) -> Self {
        Self::UnknownValue(s.as_ref().to_owned())
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Error::Custom(value)
    }
}
