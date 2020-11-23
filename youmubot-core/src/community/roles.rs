use crate::db::Roles as DB;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{channel::Message, guild::Role, id::RoleId},
    utils::MessageBuilder,
};
use youmubot_prelude::*;

#[command("listroles")]
#[description = "List all available roles in the server."]
#[num_args(0)]
#[only_in(guilds)]
async fn list(ctx: &Context, m: &Message, _: Args) -> CommandResult {
    let guild_id = m.guild_id.unwrap(); // only_in(guilds)
    let data = ctx.data.read().await;

    let db = DB::open(&*data);
    let roles = db
        .borrow()?
        .get(&guild_id)
        .filter(|v| !v.is_empty())
        .cloned();
    match roles {
        None => {
            m.reply(&ctx, "No roles available for assigning.").await?;
        }
        Some(v) => {
            let roles = guild_id.to_partial_guild(&ctx).await?.roles;
            let roles: Vec<_> = v
                .into_iter()
                .filter_map(|(_, role)| roles.get(&role.id).cloned().map(|r| (r, role.description)))
                .collect();
            const ROLES_PER_PAGE: usize = 8;
            let pages = (roles.len() + ROLES_PER_PAGE - 1) / ROLES_PER_PAGE;

            paginate_fn(
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
                        let nw = roles // name width
                            .iter()
                            .map(|(r, _)| r.name.len())
                            .max()
                            .unwrap()
                            .max(6);
                        let idw = roles[0].0.id.to_string().len();
                        let dw = roles
                            .iter()
                            .map(|v| v.1.len())
                            .max()
                            .unwrap()
                            .max(" Description ".len());
                        let mut m = MessageBuilder::new();
                        m.push_line("```");

                        // Table header
                        m.push_line(format!(
                            "{:nw$} | {:idw$} | {:dw$}",
                            "Name",
                            "ID",
                            "Description",
                            nw = nw,
                            idw = idw,
                            dw = dw,
                        ));
                        m.push_line(format!(
                            "{:->nw$}---{:->idw$}---{:->dw$}",
                            "",
                            "",
                            "",
                            nw = nw,
                            idw = idw,
                            dw = dw,
                        ));

                        for (role, description) in roles.iter() {
                            m.push_line(format!(
                                "{:nw$} | {:idw$} | {:dw$}",
                                role.name,
                                role.id,
                                description,
                                nw = nw,
                                idw = idw,
                                dw = dw,
                            ));
                        }
                        m.push_line("```");
                        m.push(format!("Page **{}/{}**", page + 1, pages));

                        msg.edit(ctx, |f| f.content(m.to_string())).await?;
                        Ok(true)
                    })
                },
                ctx,
                m.channel_id,
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
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role is not self-assignable. Check the `listroles` command to see which role can be assigned.").await?;
        }
        Some(role) => {
            let mut member = guild.member(&ctx, m.author.id).await.unwrap();
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
#[description = "Add a role as the assignable role"]
#[usage = "{role-name-or-id} / {description}"]
#[example = "hd820 / Headphones role"]
#[num_args(2)]
#[required_permissions(MANAGE_ROLES)]
#[only_in(guilds)]
async fn add(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let data = ctx.data.read().await;
    let description = args.single::<String>()?;
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx).await?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists").await?;
        }
        Some(role)
            if DB::open(&*data)
                .borrow()?
                .get(&guild_id)
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role already exists in the database.")
                .await?;
        }
        Some(role) => {
            DB::open(&*data)
                .borrow_mut()?
                .entry(guild_id)
                .or_default()
                .insert(
                    role.id,
                    crate::db::Role {
                        id: role.id,
                        description,
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
            if !DB::open(&*data)
                .borrow()?
                .get(&guild_id)
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role does not exist in the assignable list.")
                .await?;
        }
        Some(role) => {
            DB::open(&*data)
                .borrow_mut()?
                .entry(guild_id)
                .or_default()
                .remove(&role.id);
            m.react(&ctx, '👌').await?;
        }
    };
    Ok(())
}

/// Parse a string as a role.
fn role_from_string(role: &str, roles: &std::collections::HashMap<RoleId, Role>) -> Option<Role> {
    match role.parse::<u64>() {
        Ok(id) if roles.contains_key(&RoleId(id)) => roles.get(&RoleId(id)).cloned(),
        _ => roles
            .iter()
            .find_map(|(_, r)| if r.name == role { Some(r) } else { None })
            .cloned(),
    }
}
