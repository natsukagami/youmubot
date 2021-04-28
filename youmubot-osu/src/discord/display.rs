pub use beatmapset::display_beatmapset;
pub use scores::table::display_scores_table;

mod scores {
    pub mod table {
        use crate::discord::{Beatmap, BeatmapCache, BeatmapInfo, BeatmapMetaCache};
        use crate::models::{Mode, Score};
        use serenity::{framework::standard::CommandResult, model::channel::Message};
        use youmubot_prelude::*;

        pub async fn display_scores_table<'a>(
            scores: Vec<Score>,
            mode: Mode,
            ctx: &'a Context,
            m: &'a Message,
        ) -> CommandResult {
            if scores.is_empty() {
                m.reply(&ctx, "No plays found").await?;
                return Ok(());
            }

            paginate_reply(
                Paginate { scores, mode },
                ctx,
                m,
                std::time::Duration::from_secs(60),
            )
            .await?;
            Ok(())
        }

        pub struct Paginate {
            scores: Vec<Score>,
            mode: Mode,
        }

        impl Paginate {
            fn total_pages(&self) -> usize {
                (self.scores.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE
            }
        }

        const ITEMS_PER_PAGE: usize = 5;

        #[async_trait]
        impl pagination::Paginate for Paginate {
            async fn render(&mut self, page: u8, ctx: &Context, msg: &mut Message) -> Result<bool> {
                let data = ctx.data.read().await;
                let osu = data.get::<BeatmapMetaCache>().unwrap();
                let beatmap_cache = data.get::<BeatmapCache>().unwrap();
                let page = page as usize;
                let start = page * ITEMS_PER_PAGE;
                let end = self.scores.len().min(start + ITEMS_PER_PAGE);
                if start >= end {
                    return Ok(false);
                }

                let hourglass = msg.react(ctx, '⌛').await?;
                let plays = &self.scores[start..end];
                let mode = self.mode;
                let beatmaps = plays
                    .iter()
                    .map(|play| async move {
                        let beatmap = osu.get_beatmap(play.beatmap_id, mode).await?;
                        let info = {
                            let b = beatmap_cache.get_beatmap(beatmap.beatmap_id).await?;
                            mode.to_oppai_mode()
                                .and_then(|mode| b.get_info_with(Some(mode), play.mods).ok())
                        };
                        Ok((beatmap, info)) as Result<(Beatmap, Option<BeatmapInfo>)>
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.ok())
                    .collect::<Vec<_>>();
                let pp = plays
                    .iter()
                    .map(|p| async move {
                        match p.pp.map(|pp| format!("{:.2}pp", pp)) {
                            Some(v) => Ok(v),
                            None => {
                                let b = beatmap_cache.get_beatmap(p.beatmap_id).await?;
                                let r: Result<_> = Ok(mode
                                    .to_oppai_mode()
                                    .and_then(|op| {
                                        b.get_pp_from(
                                            oppai_rs::Combo::NonFC {
                                                max_combo: p.max_combo as u32,
                                                misses: p.count_miss as u32,
                                            },
                                            oppai_rs::Accuracy::from_hits(
                                                p.count_100 as u32,
                                                p.count_50 as u32,
                                            ),
                                            Some(op),
                                            p.mods,
                                        )
                                        .ok()
                                        .map(|pp| format!("{:.2}pp [?]", pp))
                                    })
                                    .unwrap_or_else(|| "-".to_owned()));
                                r
                            }
                        }
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.unwrap_or_else(|_| "-".to_owned()))
                    .collect::<Vec<String>>();
                let (beatmaps, pp) = future::join(beatmaps, pp).await;

                let ranks = plays
                    .iter()
                    .enumerate()
                    .map(|(i, p)| match p.rank {
                        crate::models::Rank::F => beatmaps[i]
                            .as_ref()
                            .and_then(|(_, i)| i.map(|i| i.objects))
                            .map(|total| {
                                (p.count_300 + p.count_100 + p.count_50 + p.count_miss) as f64
                                    / (total as f64)
                                    * 100.0
                            })
                            .map(|p| format!("F [{:.0}%]", p))
                            .unwrap_or_else(|| "F".to_owned()),
                        v => v.to_string(),
                    })
                    .collect::<Vec<_>>();

                let beatmaps = beatmaps
                    .into_iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let play = &plays[i];
                        b.map(|(beatmap, info)| {
                            format!(
                                "[{:.1}*] {} - {} [{}] ({})",
                                info.map(|i| i.stars as f64)
                                    .unwrap_or(beatmap.difficulty.stars),
                                beatmap.artist,
                                beatmap.title,
                                beatmap.difficulty_name,
                                beatmap.short_link(Some(self.mode), Some(play.mods)),
                            )
                        })
                        .unwrap_or_else(|| "FETCH_FAILED".to_owned())
                    })
                    .collect::<Vec<_>>();

                let pw = pp.iter().map(|v| v.len()).max().unwrap_or(2);
                /*mods width*/
                let mw = plays
                    .iter()
                    .map(|v| v.mods.to_string().len())
                    .max()
                    .unwrap()
                    .max(4);
                /*beatmap names*/
                let bw = beatmaps.iter().map(|v| v.len()).max().unwrap().max(7);
                /* ranks width */
                let rw = ranks.iter().map(|v| v.len()).max().unwrap().max(5);

                let mut m = serenity::utils::MessageBuilder::new();
                // Table header
                m.push_line(format!(
                    " #  | {:pw$} | accuracy | {:rw$} | {:mw$} | {:bw$}",
                    "pp",
                    "ranks",
                    "mods",
                    "beatmap",
                    rw = rw,
                    pw = pw,
                    mw = mw,
                    bw = bw
                ));
                m.push_line(format!(
                    "------{:-<pw$}--------------{:-<rw$}---{:-<mw$}---{:-<bw$}",
                    "",
                    "",
                    "",
                    "",
                    rw = rw,
                    pw = pw,
                    mw = mw,
                    bw = bw
                ));
                // Each row
                for (id, (play, beatmap)) in plays.iter().zip(beatmaps.iter()).enumerate() {
                    m.push_line(format!(
                        "{:>3} | {:>pw$} | {:>8} | {:^rw$} | {:mw$} | {:bw$}",
                        id + start + 1,
                        pp[id],
                        format!("{:.2}%", play.accuracy(self.mode)),
                        ranks[id],
                        play.mods.to_string(),
                        beatmap,
                        rw = rw,
                        pw = pw,
                        mw = mw,
                        bw = bw
                    ));
                }
                // End
                let table = m.build().replace("```", "\\`\\`\\`");
                let mut m = serenity::utils::MessageBuilder::new();
                m.push_codeblock(table, None).push_line(format!(
                    "Page **{}/{}**",
                    page + 1,
                    self.total_pages()
                ));
                if self.mode.to_oppai_mode().is_none() {
                    m.push_line("Note: star difficulty doesn't reflect mods applied.");
                } else {
                    m.push_line("[?] means pp was predicted by oppai-rs.");
                }
                msg.edit(ctx, |f| f.content(m.to_string())).await?;
                hourglass.delete(ctx).await?;
                Ok(true)
            }

            fn len(&self) -> Option<usize> {
                Some(self.total_pages())
            }
        }
    }
}

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
        fn len(&self) -> Option<usize> {
            Some(self.maps.len())
        }

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
                Some(info) => *info,
                None => {
                    let info = self.get_beatmap_info(ctx, map).await;
                    self.infos[page] = Some(info);
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
