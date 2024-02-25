use youmubot_prelude::*;

#[poise::command(slash_command)]
pub async fn example<T: AsRef<crate::Env> + Sync>(
    context: poise::Context<'_, T, Error>,
    arg: String,
) -> Result<(), Error> {
    todo!()
}
