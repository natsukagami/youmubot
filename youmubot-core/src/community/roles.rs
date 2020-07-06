use crate::db::Roles as DB;
use serenity::{
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    model::{channel::Message, guild::Role, id::RoleId},
    utils::MessageBuilder,
};
use youmubot_prelude::*;

#[command("listroles")]
#[description = "List all available roles in the server."]
#[num_args(0)]
#[only_in(guilds)]
fn list(ctx: &mut Context, m: &Message, _: Args) -> CommandResult {
    let guild_id = m.guild_id.unwrap(); // only_in(guilds)

    let db = DB::open(&*ctx.data.read());
    let db = db.borrow()?;
    let roles = db.get(&guild_id).filter(|v| !v.is_empty()).cloned();
    match roles {
        None => {
            m.reply(&ctx, "No roles available for assigning.")?;
        }
        Some(v) => {
            let roles = guild_id.to_partial_guild(&ctx)?.roles;
            let roles: Vec<_> = v
                .into_iter()
                .filter_map(|(_, role)| roles.get(&role.id).cloned().map(|r| (r, role.description)))
                .collect();
            const ROLES_PER_PAGE: usize = 8;
            let pages = (roles.len() + ROLES_PER_PAGE - 1) / ROLES_PER_PAGE;

            let watcher = ctx.data.get_cloned::<ReactionWatcher>();
            watcher.paginate_fn(
                ctx.clone(),
                m.channel_id,
                move |page, e| {
                    let page = page as usize;
                    let start = page * ROLES_PER_PAGE;
                    let end = roles.len().min(start + ROLES_PER_PAGE);
                    if end <= start {
                        return (e, Err(Error::from("No more roles to display")));
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

                    (e.content(m.build()), Ok(()))
                },
                std::time::Duration::from_secs(60 * 10),
            )?;
        }
    };
    Ok(())
}

#[command("role")]
#[description = "Toggle a role by its name or ID."]
#[example = "\"IELTS / TOEFL\""]
#[num_args(1)]
#[only_in(guilds)]
fn toggle(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx)?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists")?;
        }
        Some(role)
            if !DB::open(&*ctx.data.read())
                .borrow()?
                .get(&guild_id)
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role is not self-assignable. Check the `listroles` command to see which role can be assigned.")?;
        }
        Some(role) => {
            let mut member = m.member(&ctx).ok_or(Error::from("Cannot find member"))?;
            if member.roles.contains(&role.id) {
                member.remove_role(&ctx, &role)?;
                m.reply(&ctx, format!("Role `{}` has been removed.", role.name))?;
            } else {
                member.add_role(&ctx, &role)?;
                m.reply(&ctx, format!("Role `{}` has been assigned.", role.name))?;
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
fn add(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let description = args.single::<String>()?;
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx)?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists")?;
        }
        Some(role)
            if DB::open(&*ctx.data.read())
                .borrow()?
                .get(&guild_id)
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role already exists in the database.")?;
        }
        Some(role) => {
            DB::open(&*ctx.data.read())
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
            m.react(&ctx, "👌🏼")?;
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
fn remove(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let role = args.single_quoted::<String>()?;
    let guild_id = m.guild_id.unwrap();
    let roles = guild_id.to_partial_guild(&ctx)?.roles;
    let role = role_from_string(&role, &roles);
    match role {
        None => {
            m.reply(&ctx, "No such role exists")?;
        }
        Some(role)
            if !DB::open(&*ctx.data.read())
                .borrow()?
                .get(&guild_id)
                .map(|g| g.contains_key(&role.id))
                .unwrap_or(false) =>
        {
            m.reply(&ctx, "This role does not exist in the assignable list.")?;
        }
        Some(role) => {
            DB::open(&*ctx.data.read())
                .borrow_mut()?
                .entry(guild_id)
                .or_default()
                .remove(&role.id);
            m.react(&ctx, "👌🏼")?;
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
