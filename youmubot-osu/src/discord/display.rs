pub use beatmapset::display_beatmapset;
pub use scores::ScoreListStyle;

mod scores {
    use serenity::{framework::standard::CommandResult, model::channel::Message};

    use youmubot_prelude::*;

    use crate::models::{Mode, Score};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// The style for the scores list to be displayed.
    pub enum ScoreListStyle {
        Table,
        Grid,
    }

    impl Default for ScoreListStyle {
        fn default() -> Self {
            Self::Table
        }
    }

    impl std::str::FromStr for ScoreListStyle {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "--table" => Ok(Self::Table),
                "--grid" => Ok(Self::Grid),
                _ => Err(Error::msg("unknown value")),
            }
        }
    }

    impl ScoreListStyle {
        pub async fn display_scores<'a>(
            self,
            scores: Vec<Score>,
            mode: Mode,
            ctx: &'a Context,
            m: &'a Message,
        ) -> CommandResult {
            match self {
                ScoreListStyle::Table => table::display_scores_table(scores, mode, ctx, m).await,
                ScoreListStyle::Grid => grid::display_scores_grid(scores, mode, ctx, m).await,
            }
        }
    }

    pub mod grid {
        use serenity::{framework::standard::CommandResult, model::channel::Message};
        use serenity::builder::EditMessage;

        use youmubot_prelude::*;

        use crate::discord::{
            BeatmapCache, BeatmapMetaCache, BeatmapWithMode, cache::save_beatmap,
        };
        use crate::models::{Mode, Score};

        pub async fn display_scores_grid<'a>(
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

        #[async_trait]
        impl pagination::Paginate for Paginate {
            async fn render(&mut self, page: u8, ctx: &Context, msg: &mut Message) -> Result<bool> {
                let data = ctx.data.read().await;
                let client = data.get::<crate::discord::OsuClient>().unwrap();
                let osu = data.get::<BeatmapMetaCache>().unwrap();
                let beatmap_cache = data.get::<BeatmapCache>().unwrap();
                let page = page as usize;
                let score = &self.scores[page];

                let hourglass = msg.react(ctx, '⌛').await?;
                let mode = self.mode;
                let beatmap = osu.get_beatmap(score.beatmap_id, mode).await?;
                let content = beatmap_cache.get_beatmap(beatmap.beatmap_id).await?;
                let bm = BeatmapWithMode(beatmap, mode);
                let user = client
                    .user(crate::request::UserID::ID(score.user_id), |f| f)
                    .await?
                    .ok_or_else(|| Error::msg("user not found"))?;

                msg.edit(
                    ctx,
                    EditMessage::new().embed({
                        crate::discord::embeds::score_embed(score, &bm, &content, &user)
                            .footer(format!("Page {}/{}", page + 1, self.scores.len()))
                            .build()
                    }),
                )
                    .await?;
                save_beatmap(&*ctx.data.read().await, msg.channel_id, &bm).await?;

                // End
                hourglass.delete(ctx).await?;
                Ok(true)
            }

            fn len(&self) -> Option<usize> {
                Some(self.scores.len())
            }
        }
    }

    pub mod table {
        use std::borrow::Cow;

        use serenity::{framework::standard::CommandResult, model::channel::Message};
        use serenity::builder::EditMessage;

        use youmubot_prelude::*;
        use youmubot_prelude::table_format::{Align, table_formatting};
        use youmubot_prelude::table_format::Align::{Left, Right};

        use crate::discord::{Beatmap, BeatmapCache, BeatmapInfo, BeatmapMetaCache};
        use crate::discord::oppai_cache::Accuracy;
        use crate::models::{Mode, Score};

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
                            b.get_info_with(mode, play.mods).ok()
                        };
                        Ok((beatmap, info)) as Result<(Beatmap, Option<BeatmapInfo>)>
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.ok())
                    .collect::<Vec<_>>();

                let pps = plays
                    .iter()
                    .map(|p| async move {
                        match p.pp.map(|pp| format!("{:.2}", pp)) {
                            Some(v) => Ok(v),
                            None => {
                                let b = beatmap_cache.get_beatmap(p.beatmap_id).await?;
                                let r: Result<_> = Ok({
                                    b.get_pp_from(
                                        mode,
                                        Some(p.max_combo as usize),
                                        Accuracy::ByCount(
                                            p.count_300,
                                            p.count_100,
                                            p.count_50,
                                            p.count_miss,
                                        ),
                                        p.mods,
                                    )
                                        .ok()
                                        .map(|pp| format!("{:.2}[?]", pp))
                                }
                                    .unwrap_or_else(|| "-".to_owned()));
                                r
                            }
                        }
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v| v.unwrap_or_else(|_| "-".to_owned()))
                    .collect::<Vec<String>>();

                let (beatmaps, pps) = future::join(beatmaps, pps).await;

                let ranks = plays
                    .iter()
                    .enumerate()
                    .map(|(i, p)| -> Cow<'static, str> {
                        match p.rank {
                            crate::models::Rank::F => beatmaps[i]
                                .as_ref()
                                .and_then(|(_, i)| i.map(|i| i.objects))
                                .map(|total| {
                                    (p.count_300 + p.count_100 + p.count_50 + p.count_miss) as f64
                                        / (total as f64)
                                        * 100.0
                                })
                                .map(|p| format!("{:.0}% F", p).into())
                                .unwrap_or_else(|| "F".into()),
                            crate::models::Rank::SS => "SS".into(),
                            crate::models::Rank::S => if p.perfect {
                                format!("{}x FC S", p.max_combo)
                            } else {
                                format!("{}x S", p.max_combo)
                            }
                                .into(),
                            _v => format!("{}x {}m {}", p.max_combo, p.count_miss, p.rank).into(),
                        }
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
                                info.map(|i| i.stars).unwrap_or(beatmap.difficulty.stars),
                                beatmap.artist,
                                beatmap.title,
                                beatmap.difficulty_name,
                                beatmap.short_link(Some(self.mode), Some(play.mods)),
                            )
                        })
                            .unwrap_or_else(|| "FETCH_FAILED".to_owned())
                    })
                    .collect::<Vec<_>>();

                const SCORE_HEADERS: [&'static str; 6] =
                    ["#", "PP", "Acc", "Ranks", "Mods", "Beatmap"];
                const SCORE_ALIGNS: [Align; 6] = [Right, Right, Right, Right, Right, Left];

                let score_arr = plays
                    .iter()
                    .zip(beatmaps.iter())
                    .zip(ranks.iter().zip(pps.iter()))
                    .enumerate()
                    .map(|(id, ((play, beatmap), (rank, pp)))| {
                        [
                            format!("{}", id + start + 1),
                            format!("{}", pp),
                            format!("{:.2}%", play.accuracy(self.mode)),
                            format!("{}", rank),
                            play.mods.to_string(),
                            beatmap.clone(),
                        ]
                    })
                    .collect::<Vec<_>>();

                let score_table = table_formatting(&SCORE_HEADERS, &SCORE_ALIGNS, score_arr);

                let content = serenity::utils::MessageBuilder::new()
                    .push_line(score_table)
                    .push_line(format!("Page **{}/{}**", page + 1, self.total_pages()))
                    .push_line("[?] means pp was predicted by oppai-rs.")
                    .build();

                msg.edit(ctx, EditMessage::new().content(content)).await?;
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
    use serenity::{
        all::Reaction,
        builder::{CreateEmbedFooter, EditMessage},
        model::channel::Message,
        model::channel::ReactionType,
    };

    use youmubot_prelude::*;

    use crate::{
        discord::{
            BeatmapCache, BeatmapWithMode, cache::save_beatmap, oppai_cache::BeatmapInfoWithPP,
        },
        models::{Beatmap, Mode, Mods},
    };

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
        infos: Vec<Option<BeatmapInfoWithPP>>,
        mode: Option<Mode>,
        mods: Mods,
        message: String,
    }

    impl Paginate {
        async fn get_beatmap_info(&self, ctx: &Context, b: &Beatmap) -> Result<BeatmapInfoWithPP> {
            let data = ctx.data.read().await;
            let cache = data.get::<BeatmapCache>().unwrap();
            cache
                .get_beatmap(b.beatmap_id)
                .await
                .and_then(move |v| v.get_possible_pp_with(self.mode.unwrap_or(b.mode), self.mods))
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
                m.edit(
                    ctx,
                    EditMessage::new().embed(crate::discord::embeds::beatmapset_embed(
                        &self.maps[..],
                        self.mode,
                    )),
                )
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
                    let info = self.get_beatmap_info(ctx, map).await?;
                    self.infos[page] = Some(info);
                    info
                }
            };
            m.edit(ctx,
                   EditMessage::new().content(self.message.as_str()).embed(
                       crate::discord::embeds::beatmap_embed(
                           map,
                           self.mode.unwrap_or(map.mode),
                           self.mods,
                           info,
                       )
                           .footer({
                               CreateEmbedFooter::new(format!(
                                   "Difficulty {}/{}. To show all difficulties in a single embed (old style), react {}",
                                   page + 1,
                                   self.maps.len(),
                                   SHOW_ALL_EMOTE,
                               ))
                           })
                   ),
            )
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
            reaction: &Reaction,
        ) -> Result<Option<u8>> {
            // Render the old style.
            if let ReactionType::Unicode(s) = &reaction.emoji {
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
