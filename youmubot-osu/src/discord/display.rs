pub use beatmapset::display_beatmapset;

mod beatmapset {
    use crate::{
        discord::{oppai_cache::BeatmapInfo, BeatmapCache, OsuClient},
        models::{Beatmap, Mode, Mods},
        request::BeatmapRequestKind,
    };
    use serenity::{
        collector::ReactionAction, model::channel::Message, model::channel::ReactionType,
    };
    use youmubot_prelude::*;

    const SHOW_ALL_EMOTE: &str = "🗒️";

    pub async fn display_beatmapset(
        ctx: &Context,
        beatmapset_id: u64,
        mode: Option<Mode>,
        mods: Option<Mods>,
        reply_to: &Message,
        message: impl AsRef<str>,
    ) -> Result<bool> {
        let data = ctx.data.read().await;
        let client = data.get::<OsuClient>().unwrap();
        let mods = mods.unwrap_or(Mods::NOMOD);

        let beatmapset = client
            .beatmaps(BeatmapRequestKind::Beatmapset(beatmapset_id), |f| {
                if let Some(mode) = mode {
                    f.mode(mode, true);
                }
                f
            })
            .await?;
        // Try and collect beatmap info
        let beatmap_infos = {
            let cache = data.get::<BeatmapCache>().unwrap();
            beatmapset
                .iter()
                .map(|b| {
                    let mode = b.mode.to_oppai_mode();
                    cache.get_beatmap(b.beatmap_id).map(move |v| {
                        v.ok()
                            .and_then(move |v| v.get_info_with(Some(mode?), mods).ok())
                    })
                })
                .collect::<stream::FuturesOrdered<_>>()
                .collect::<Vec<_>>()
                .await
        };

        if beatmapset.is_empty() {
            return Ok(false);
        }

        let p = Paginate {
            maps: beatmapset,
            infos: beatmap_infos,
            mode,
            mods,
            owner: reply_to.author.id,
            message: message.as_ref().to_owned(),
        };

        pagination::paginate_reply(p, ctx, reply_to, std::time::Duration::from_secs(60)).await?;
        Ok(true)
    }

    struct Paginate {
        maps: Vec<Beatmap>,
        infos: Vec<Option<BeatmapInfo>>,
        mode: Option<Mode>,
        mods: Mods,
        owner: serenity::model::id::UserId,
        message: String,
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
            if page >= self.maps.len() {
                return Ok(false);
            }

            let map = &self.maps[page];
            let info = self.infos[page].clone();
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
            if let ReactionAction::Added(v) = reaction {
                if let ReactionType::Unicode(s) = &v.emoji {
                    if s == SHOW_ALL_EMOTE && v.user_id.filter(|&id| id == self.owner).is_some() {
                        message
                            .edit(ctx, |f| {
                                f.embed(|em| {
                                    crate::discord::embeds::beatmapset_embed(
                                        &self.maps[..],
                                        self.mode,
                                        em,
                                    )
                                })
                            })
                            .await?;
                        return Ok(None);
                    }
                }
            }
            pagination::handle_pagination_reaction(page, self, ctx, message, reaction)
                .await
                .map(Some)
        }
    }
}
