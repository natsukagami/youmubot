pub use beatmapset::display_beatmapset;

mod beatmapset {
    use crate::{
        discord::{
            cache::save_beatmap, oppai_cache::BeatmapInfoWithPP, BeatmapCache, BeatmapWithMode,
        },
        models::{Beatmap, Mode, Mods},
    };
    use serenity::{
        collector::ReactionAction, model::channel::Message, model::channel::ReactionType,
    };
    use youmubot_prelude::*;

    const SHOW_ALL_EMOTE: &str = "🗒️";

    pub async fn display_beatmapset(
        ctx: &Context,
        beatmapset: Vec<Beatmap>,
        mode: Option<Mode>,
        mods: Option<Mods>,
        reply_to: &Message,
        message: impl AsRef<str>,
    ) -> Result<bool> {
        let mods = mods.unwrap_or(Mods::NOMOD);

        if beatmapset.is_empty() {
            return Ok(false);
        }

        let p = Paginate {
            infos: vec![None; beatmapset.len()],
            maps: beatmapset,
            mode,
            mods,
            message: message.as_ref().to_owned(),
        };

        let ctx = ctx.clone();
        let reply_to = reply_to.clone();
        spawn_future(async move {
            pagination::paginate_reply(p, &ctx, &reply_to, std::time::Duration::from_secs(60))
                .await
                .pls_ok();
        });
        Ok(true)
    }

    struct Paginate {
        maps: Vec<Beatmap>,
        infos: Vec<Option<Option<BeatmapInfoWithPP>>>,
        mode: Option<Mode>,
        mods: Mods,
        message: String,
    }

    impl Paginate {
        async fn get_beatmap_info(&self, ctx: &Context, b: &Beatmap) -> Option<BeatmapInfoWithPP> {
            let data = ctx.data.read().await;
            let cache = data.get::<BeatmapCache>().unwrap();
            let mode = self.mode.unwrap_or(b.mode).to_oppai_mode();
            cache
                .get_beatmap(b.beatmap_id)
                .map(move |v| {
                    v.ok()
                        .and_then(move |v| v.get_possible_pp_with(Some(mode?), self.mods).ok())
                })
                .await
        }
    }

    #[async_trait]
    impl pagination::Paginate for Paginate {
        async fn render(
            &mut self,
            page: u8,
            ctx: &Context,
            m: &mut serenity::model::channel::Message,
        ) -> Result<bool> {
            let page = page as usize;
            if page == self.maps.len() {
                m.edit(ctx, |f| {
                    f.embed(|em| {
                        crate::discord::embeds::beatmapset_embed(&self.maps[..], self.mode, em)
                    })
                })
                .await?;
                return Ok(true);
            }
            if page > self.maps.len() {
                return Ok(false);
            }

            let map = &self.maps[page];
            let info = match &self.infos[page] {
                Some(info) => info.clone(),
                None => {
                    let info = self.get_beatmap_info(ctx, map).await;
                    self.infos[page] = Some(info.clone());
                    info
                }
            };
            m.edit(ctx, |e| {
                e.content(self.message.as_str()).embed(|em| {
                    crate::discord::embeds::beatmap_embed(
                        map,
                        self.mode.unwrap_or(map.mode),
                        self.mods,
                        info,
                        em,
                    )
                    .footer(|f| {
                        f.text(format!(
                            "Difficulty {}/{}. To show all difficulties in a single embed (old style), react {}",
                            page + 1,
                            self.maps.len(),
                            SHOW_ALL_EMOTE,
                        ))
                    })
                })
            })
            .await?;
            save_beatmap(
                &*ctx.data.read().await,
                m.channel_id,
                &BeatmapWithMode(map.clone(), self.mode.unwrap_or(map.mode)),
            )
            .await
            .pls_ok();

            Ok(true)
        }

        async fn prerender(
            &mut self,
            ctx: &Context,
            m: &mut serenity::model::channel::Message,
        ) -> Result<()> {
            m.react(&ctx, SHOW_ALL_EMOTE.parse::<ReactionType>().unwrap())
                .await?;
            Ok(())
        }

        async fn handle_reaction(
            &mut self,
            page: u8,
            ctx: &Context,
            message: &mut serenity::model::channel::Message,
            reaction: &ReactionAction,
        ) -> Result<Option<u8>> {
            // Render the old style.
            let v = match reaction {
                ReactionAction::Added(v) | ReactionAction::Removed(v) => v,
            };
            if let ReactionType::Unicode(s) = &v.emoji {
                if s == SHOW_ALL_EMOTE {
                    self.render(self.maps.len() as u8, ctx, message).await?;
                    return Ok(Some(self.maps.len() as u8));
                }
            }
            pagination::handle_pagination_reaction(page, self, ctx, message, reaction)
                .await
                .map(Some)
        }
    }
}
