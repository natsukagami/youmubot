pub use beatmapset::display_beatmapset;
pub use scores::ScoreListStyle;

mod scores {
    use poise::ChoiceParameter;
    use serenity::{all::GuildId, model::channel::Message};

    use youmubot_prelude::*;

    use crate::models::Score;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ChoiceParameter)]
    /// The style for the scores list to be displayed.
    pub enum ScoreListStyle {
        #[name = "ASCII Table"]
        Table,
        #[name = "List of Embeds"]
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
        pub async fn display_scores(
            self,
            scores: Vec<Score>,
            ctx: &Context,
            guild_id: Option<GuildId>,
            m: Message,
        ) -> Result<()> {
            match self {
                ScoreListStyle::Table => table::display_scores_table(scores, ctx, m).await,
                ScoreListStyle::Grid => grid::display_scores_grid(scores, ctx, guild_id, m).await,
            }
        }
    }

    mod grid {
        use pagination::paginate_with_first_message;
        use serenity::all::GuildId;
        use serenity::builder::EditMessage;
        use serenity::model::channel::Message;

        use youmubot_prelude::*;

        use crate::discord::interaction::score_components;
        use crate::discord::{cache::save_beatmap, BeatmapWithMode, OsuEnv};
        use crate::models::Score;

        pub async fn display_scores_grid(
            scores: Vec<Score>,
            ctx: &Context,
            guild_id: Option<GuildId>,
            mut on: Message,
        ) -> Result<()> {
            if scores.is_empty() {
                on.edit(&ctx, EditMessage::new().content("No plays found"))
                    .await?;
                return Ok(());
            }

            paginate_with_first_message(
                Paginate { scores, guild_id },
                ctx,
                on,
                std::time::Duration::from_secs(60),
            )
            .await?;
            Ok(())
        }

        pub struct Paginate {
            scores: Vec<Score>,
            guild_id: Option<GuildId>,
        }

        #[async_trait]
        impl pagination::Paginate for Paginate {
            async fn render(
                &mut self,
                page: u8,
                ctx: &Context,
                msg: &Message,
            ) -> Result<Option<EditMessage>> {
                let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
                let page = page as usize;
                let score = &self.scores[page];

                let beatmap = env
                    .beatmaps
                    .get_beatmap(score.beatmap_id, score.mode)
                    .await?;
                let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
                let mode = if beatmap.mode == score.mode {
                    None
                } else {
                    Some(score.mode)
                };
                let bm = BeatmapWithMode(beatmap, mode);
                let user = env
                    .client
                    .user(&crate::request::UserID::ID(score.user_id), |f| f)
                    .await?
                    .ok_or_else(|| Error::msg("user not found"))?;

                save_beatmap(&env, msg.channel_id, &bm).await?;
                Ok(Some(
                    EditMessage::new()
                        .embed({
                            crate::discord::embeds::score_embed(score, &bm, &content, &user)
                                .footer(format!("Page {}/{}", page + 1, self.scores.len()))
                                .build()
                        })
                        .components(vec![score_components(self.guild_id), self.pagination_row()]),
                ))
            }

            fn len(&self) -> Option<usize> {
                Some(self.scores.len())
            }
        }
    }

    pub mod table {
        use std::borrow::Cow;

        use pagination::paginate_with_first_message;
        use serenity::builder::EditMessage;
        use serenity::model::channel::Message;

        use youmubot_prelude::table_format::Align::{Left, Right};
        use youmubot_prelude::table_format::{table_formatting, Align};
        use youmubot_prelude::*;

        use crate::discord::oppai_cache::Stats;
        use crate::discord::{time_before_now, Beatmap, BeatmapInfo, OsuEnv};
        use crate::models::Score;

        pub async fn display_scores_table(
            scores: Vec<Score>,
            ctx: &Context,
            mut on: Message,
        ) -> Result<()> {
            if scores.is_empty() {
                on.edit(&ctx, EditMessage::new().content("No plays found"))
                    .await?;
                return Ok(());
            }

            paginate_with_first_message(
                Paginate {
                    header: on.content.clone(),
                    scores,
                },
                ctx,
                on,
                std::time::Duration::from_secs(60),
            )
            .await?;
            Ok(())
        }

        pub struct Paginate {
            header: String,
            scores: Vec<Score>,
        }

        impl Paginate {
            fn total_pages(&self) -> usize {
                (self.scores.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE
            }
        }

        const ITEMS_PER_PAGE: usize = 5;

        #[async_trait]
        impl pagination::Paginate for Paginate {
            async fn render(
                &mut self,
                page: u8,
                ctx: &Context,
                _: &Message,
            ) -> Result<Option<EditMessage>> {
                let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

                let meta_cache = &env.beatmaps;
                let oppai = &env.oppai;
                let page = page as usize;
                let start = page * ITEMS_PER_PAGE;
                let end = self.scores.len().min(start + ITEMS_PER_PAGE);
                if start >= end {
                    return Ok(None);
                }

                let plays = &self.scores[start..end];
                let beatmaps = plays
                    .iter()
                    .map(|play| async move {
                        let beatmap = meta_cache.get_beatmap(play.beatmap_id, play.mode).await?;
                        let info = {
                            let b = oppai.get_beatmap(beatmap.beatmap_id).await?;
                            b.get_info_with(play.mode, &play.mods)
                        };
                        Ok((beatmap, info)) as Result<(Beatmap, BeatmapInfo)>
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
                                let b = oppai.get_beatmap(p.beatmap_id).await?;
                                let pp = b.get_pp_from(
                                    p.mode,
                                    Some(p.max_combo),
                                    Stats::Raw(&p.statistics),
                                    &p.mods,
                                );
                                Ok(format!("{:.2}[?]", pp))
                            }
                        }
                    })
                    .collect::<stream::FuturesOrdered<_>>()
                    .map(|v: Result<_>| v.unwrap_or_else(|_| "-".to_owned()))
                    .collect::<Vec<String>>();

                let (beatmaps, pps) = future::join(beatmaps, pps).await;

                let ranks = plays
                    .iter()
                    .enumerate()
                    .map(|(i, p)| -> Cow<'static, str> {
                        match p.rank {
                            crate::models::Rank::F => beatmaps[i]
                                .as_ref()
                                .map(|(_, i)| i.object_count)
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
                                info.attrs.stars(),
                                beatmap.artist,
                                beatmap.title,
                                beatmap.difficulty_name,
                                beatmap.short_link(Some(play.mode), &play.mods),
                            )
                        })
                        .unwrap_or_else(|| "FETCH_FAILED".to_owned())
                    })
                    .collect::<Vec<_>>();

                const SCORE_HEADERS: [&str; 7] =
                    ["#", "PP", "Acc", "Ranks", "Mods", "When", "Beatmap"];
                const SCORE_ALIGNS: [Align; 7] = [Right, Right, Right, Right, Right, Right, Left];

                let score_arr = plays
                    .iter()
                    .zip(beatmaps.iter())
                    .zip(ranks.iter().zip(pps.iter()))
                    .enumerate()
                    .map(|(id, ((play, beatmap), (rank, pp)))| {
                        [
                            format!("{}", id + start + 1),
                            pp.to_string(),
                            format!("{:.2}%", play.accuracy(play.mode)),
                            format!("{}", rank),
                            play.mods.to_string(),
                            time_before_now(&play.date),
                            beatmap.clone(),
                        ]
                    })
                    .collect::<Vec<_>>();

                let score_table = table_formatting(&SCORE_HEADERS, &SCORE_ALIGNS, score_arr);

                let content = serenity::utils::MessageBuilder::new()
                    .push_line(&self.header)
                    .push_line(score_table)
                    .push_line(format!("Page **{}/{}**", page + 1, self.total_pages()))
                    .push_line("[?] means pp was predicted by oppai-rs.")
                    .build();

                Ok(Some(
                    EditMessage::new()
                        .content(content)
                        .components(vec![self.pagination_row()]),
                ))
            }

            fn len(&self) -> Option<usize> {
                Some(self.total_pages())
            }
        }
    }
}

mod beatmapset {
    use serenity::{
        all::{CreateButton, GuildId},
        builder::{CreateEmbedFooter, EditMessage},
        model::channel::{Message, ReactionType},
    };

    use youmubot_prelude::*;

    use crate::{
        discord::{cache::save_beatmap, oppai_cache::BeatmapInfoWithPP, BeatmapWithMode},
        models::{Beatmap, Mode, Mods},
    };
    use crate::{
        discord::{interaction::beatmap_components, OsuEnv},
        mods::UnparsedMods,
    };

    const SHOW_ALL_EMOTE: &str = "🗒️";
    const SHOW_ALL: &str = "youmubot_osu::discord::display::show_all";

    pub async fn display_beatmapset(
        ctx: Context,
        mut beatmapset: Vec<Beatmap>,
        mode: Option<Mode>,
        mods: Option<UnparsedMods>,
        guild_id: Option<GuildId>,
        target: Message,
    ) -> Result<bool> {
        assert!(!beatmapset.is_empty(), "Beatmapset should not be empty");

        beatmapset.sort_unstable_by(|a, b| {
            if a.mode != b.mode {
                (a.mode as u8).cmp(&(b.mode as u8))
            } else {
                a.difficulty.stars.partial_cmp(&b.difficulty.stars).unwrap()
            }
        });

        let p = Paginate {
            infos: vec![None; beatmapset.len()],
            maps: beatmapset,
            mode,
            mods,
            guild_id,
        };

        let ctx = ctx.clone();
        spawn_future(async move {
            pagination::paginate_with_first_message(
                p,
                &ctx,
                target,
                std::time::Duration::from_secs(60),
            )
            .await
            .pls_ok();
        });
        Ok(true)
    }

    struct Paginate {
        maps: Vec<Beatmap>,
        infos: Vec<Option<BeatmapInfoWithPP>>,
        mode: Option<Mode>,
        mods: Option<UnparsedMods>,
        guild_id: Option<GuildId>,
    }

    impl Paginate {
        async fn get_beatmap_info(
            &self,
            ctx: &Context,
            b: &Beatmap,
            mods: &Mods,
        ) -> Result<BeatmapInfoWithPP> {
            let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();

            env.oppai
                .get_beatmap(b.beatmap_id)
                .await
                .map(move |v| v.get_possible_pp_with(b.mode.with_override(self.mode), &mods))
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
            msg: &Message,
        ) -> Result<Option<EditMessage>> {
            let page = page as usize;
            if page == self.maps.len() {
                return Ok(Some(
                    EditMessage::new()
                        .embed(crate::discord::embeds::beatmapset_embed(
                            &self.maps[..],
                            self.mode,
                        ))
                        .components(vec![self.pagination_row()]),
                ));
            }
            if page > self.maps.len() {
                return Ok(None);
            }

            let map = &self.maps[page];
            let mods = self
                .mods
                .clone()
                .and_then(|v| v.to_mods(map.mode.with_override(self.mode)).ok())
                .unwrap_or_default();

            let info = match &self.infos[page] {
                Some(info) => info,
                None => {
                    let info = self.get_beatmap_info(ctx, map, &mods).await?;
                    self.infos[page].insert(info)
                }
            };
            let env = ctx.data.read().await.get::<OsuEnv>().unwrap().clone();
            save_beatmap(
                &env,
                msg.channel_id,
                &BeatmapWithMode(map.clone(), self.mode),
            )
            .await
            .pls_ok();

            Ok(Some(
                     EditMessage::new().embed(
                       crate::discord::embeds::beatmap_embed(
                           map,
                           self.mode.unwrap_or(map.mode),
                           &mods,
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
                   )
                   .components(vec![beatmap_components(map.mode, self.guild_id), self.pagination_row()]),
            ))
        }

        fn interaction_buttons(&self) -> Vec<CreateButton> {
            let mut btns = pagination::default_buttons(self);
            btns.insert(
                0,
                CreateButton::new(SHOW_ALL)
                    .emoji(ReactionType::try_from(SHOW_ALL_EMOTE).unwrap())
                    .label("Show all"),
            );
            btns
        }

        async fn handle_reaction(
            &mut self,
            page: u8,
            ctx: &Context,
            message: &mut serenity::model::channel::Message,
            reaction: &str,
        ) -> Result<Option<u8>> {
            // Render the old style.
            if reaction == SHOW_ALL {
                pagination::do_render(self, self.maps.len() as u8, ctx, message).await?;
                return Ok(Some(self.maps.len() as u8));
            }
            pagination::handle_pagination_reaction(page, self, ctx, message, reaction)
                .await
                .map(Some)
        }
    }
}
