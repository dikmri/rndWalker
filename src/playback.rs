use crate::app::{bucket_target_size, PreloadState, PreparedVideo, RndWalkerApp};
use crate::loader::{LoadPurpose, LoadRequest};
use crate::media::choose_random_path_avoiding_recent;
use eframe::egui::{self, Color32, Context, Frame, Layout, Rect, RichText, Vec2};
use egui_video::{AudioDevice, Player, PlayerState};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub(crate) const VIDEO_PRELOAD_BEFORE_END_MS: i64 = 2_500;
pub(crate) const VIDEO_PRELOAD_WARMUP_MS: i64 = 140;
pub(crate) const VIDEO_PRELOAD_WARMUP_TIMEOUT_MS: u64 = 700;
pub(crate) const VIDEO_FINISH_GRACE_MS: i64 = 30;
pub(crate) const RANDOM_RECENT_EXCLUSION_COUNT: usize = 3;

pub(crate) fn fit_rect(container: Rect, content_size: Vec2) -> Rect {
    if content_size.x <= 0.0 || content_size.y <= 0.0 {
        return container;
    }

    let scale = (container.width() / content_size.x).min(container.height() / content_size.y);
    Rect::from_center_size(container.center(), content_size * scale)
}

impl RndWalkerApp {
    pub(crate) fn play_next_video(&mut self, _ctx: &Context, add_to_history: bool) {
        let videos = self.library.active_videos(self.active_folder);
        let Some(path) = self.choose_next_video_path(&videos) else {
            self.show_info("再生できるMP4ファイルがありません");
            return;
        };

        self.load_video(_ctx, path, add_to_history);
    }

    /// Enqueue a background load for `path` as the new active video. The currently playing video
    /// keeps running until the result arrives (no UI freeze). A fresh request id supersedes any
    /// earlier in-flight Single request.
    pub(crate) fn load_video(&mut self, _ctx: &Context, path: PathBuf, add_to_history: bool) {
        // A new active load makes any queued preload irrelevant.
        self.clear_queued_video();
        self.queued_in_flight = false;

        let request_id = self.next_single_request_id;
        self.next_single_request_id = self.next_single_request_id.wrapping_add(1);
        self.pending_single = Some(request_id);

        self.loader.enqueue(LoadRequest {
            generation: self.loader_generation,
            purpose: LoadPurpose::Single {
                add_to_history,
                request_id,
            },
            path,
            target_size: Some(self.last_player_target_size.unwrap_or((0, 0))),
            economy: false,
            attempts: 0,
        });
    }

    /// Main-thread handler for a finished `Single` load. Ignores superseded requests; otherwise
    /// attaches audio on the main thread and promotes the player to active.
    pub(crate) fn on_single_loaded(
        &mut self,
        request_id: u64,
        path: PathBuf,
        add_to_history: bool,
        player: Result<Player, String>,
    ) {
        if self.pending_single != Some(request_id) {
            // A later press already superseded this load; discard it.
            if let Ok(mut player) = player {
                player.stop();
            }
            return;
        }
        self.pending_single = None;

        let mut player = match player {
            Ok(player) => player,
            Err(error) => {
                self.show_info(format!("動画を読み込めません: {error}"));
                return;
            }
        };

        let (audio_device, warning) = self.attach_video_audio(&mut player);

        let prepared = PreparedVideo {
            path,
            player,
            audio_device,
            warning,
            preload_state: PreloadState::Cold,
        };
        self.start_prepared_video(prepared, add_to_history);
    }

    pub(crate) fn stop_video(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.stop();
        }
        self.player = None;
        self.video_audio_device = None;
        self.current_video = None;
    }

    pub(crate) fn clear_queued_video(&mut self) {
        if let Some(mut queued_video) = self.queued_video.take() {
            queued_video.player.stop();
        }
        self.queued_video_folder = None;
    }

    /// Attach an audio device to a worker-built player. SDL audio must be created on the main
    /// thread, so this runs here (not on the loader worker). Returns the device (kept alive
    /// alongside the player) and an optional warning to surface to the user.
    pub(crate) fn attach_video_audio(
        &self,
        player: &mut Player,
    ) -> (Option<AudioDevice>, Option<String>) {
        match AudioDevice::new() {
            Ok(mut audio_device) => {
                if let Err(error) = player.add_audio(&mut audio_device) {
                    (None, Some(format!("動画音声を初期化できません: {error}")))
                } else {
                    (Some(audio_device), None)
                }
            }
            Err(error) => (
                None,
                Some(format!("動画音声デバイスを初期化できません: {error}")),
            ),
        }
    }

    pub(crate) fn start_prepared_video(&mut self, prepared: PreparedVideo, add_to_history: bool) {
        self.stop_video();

        let PreparedVideo {
            path,
            mut player,
            audio_device,
            warning,
            preload_state,
        } = prepared;

        player.options.set_audio_volume(self.effective_volume());
        // The request may have been enqueued before the first frame established a target size
        // (decoding at native resolution); apply the current window cap on promotion.
        if let Some((width, height)) = self.last_player_target_size {
            player.set_target_texture_size(width, height);
        }
        match preload_state {
            PreloadState::Ready => player.resume(),
            PreloadState::Warming { .. } if player.elapsed_ms() <= VIDEO_PRELOAD_WARMUP_MS * 2 => {
                player.resume();
            }
            PreloadState::Cold | PreloadState::Warming { .. } | PreloadState::SeekingToStart => {
                player.start();
            }
        }
        self.current_video = Some(path.clone());
        self.video_audio_device = audio_device;
        self.player = Some(player);

        if add_to_history {
            if let Some(index) = self.history_index {
                self.history.truncate(index + 1);
            }
            self.history.push(path);
            self.history_index = self.history.len().checked_sub(1);
        }

        if let Some(warning) = warning {
            self.show_info(warning);
        }
    }

    pub(crate) fn target_folder_for_next_video(&self) -> Option<usize> {
        self.pending_folder.unwrap_or(self.active_folder)
    }

    pub(crate) fn recent_video_paths(&self) -> Vec<PathBuf> {
        let end = self
            .history_index
            .map(|index| index + 1)
            .unwrap_or(self.history.len())
            .min(self.history.len());

        self.history[..end]
            .iter()
            .rev()
            .take(RANDOM_RECENT_EXCLUSION_COUNT)
            .cloned()
            .collect()
    }

    pub(crate) fn choose_next_video_path(&self, videos: &[PathBuf]) -> Option<PathBuf> {
        choose_random_path_avoiding_recent(
            videos,
            &self.recent_video_paths(),
            RANDOM_RECENT_EXCLUSION_COUNT,
        )
    }

    pub(crate) fn ensure_next_video_preloaded(&mut self, _ctx: &Context) {
        if self.queued_video.is_some() || self.queued_in_flight {
            return;
        }

        let target_folder = self.target_folder_for_next_video();
        let videos = self.library.active_videos(target_folder);
        let Some(path) = self.choose_next_video_path(&videos) else {
            return;
        };

        self.queued_in_flight = true;
        self.loader.enqueue(LoadRequest {
            generation: self.loader_generation,
            purpose: LoadPurpose::Queued {
                folder: target_folder,
            },
            path,
            target_size: Some(self.last_player_target_size.unwrap_or((0, 0))),
            economy: false,
            attempts: 0,
        });
    }

    /// Main-thread handler for a finished `Queued` preload. Builds the [`PreparedVideo`], attaches
    /// muted audio, warms it up, and stores it for the next advance.
    pub(crate) fn on_queued_loaded(
        &mut self,
        path: PathBuf,
        folder: Option<usize>,
        player: Result<Player, String>,
    ) {
        self.queued_in_flight = false;

        let mut player = match player {
            Ok(player) => player,
            Err(_) => return,
        };

        let (audio_device, warning) = self.attach_video_audio(&mut player);
        let mut prepared = PreparedVideo {
            path,
            player,
            audio_device,
            warning,
            preload_state: PreloadState::Cold,
        };
        self.start_queued_video_warmup(&mut prepared);
        self.queued_video = Some(prepared);
        self.queued_video_folder = folder;
    }

    pub(crate) fn start_queued_video_warmup(&self, prepared: &mut PreparedVideo) {
        prepared.player.options.set_audio_volume(0.0);
        prepared.player.start();
        prepared.preload_state = PreloadState::Warming {
            started_at: Instant::now(),
        };
    }

    pub(crate) fn service_queued_video(&mut self) {
        let Some(prepared) = self.queued_video.as_mut() else {
            return;
        };

        prepared.player.options.set_audio_volume(0.0);
        prepared.player.process_state();

        match prepared.preload_state {
            PreloadState::Cold => {}
            PreloadState::Warming { started_at } => {
                let warmed_by_frame = prepared.player.elapsed_ms() >= VIDEO_PRELOAD_WARMUP_MS;
                let timed_out =
                    started_at.elapsed() >= Duration::from_millis(VIDEO_PRELOAD_WARMUP_TIMEOUT_MS);

                if warmed_by_frame || timed_out {
                    prepared.player.pause();
                    prepared.player.seek(0.0);
                    prepared.preload_state = PreloadState::SeekingToStart;
                }
            }
            PreloadState::SeekingToStart => {
                if matches!(prepared.player.player_state.get(), PlayerState::Paused) {
                    prepared.preload_state = PreloadState::Ready;
                }
            }
            PreloadState::Ready => {}
        }
    }

    pub(crate) fn advance_after_video_finished(&mut self, ctx: &Context) {
        self.apply_pending_folder_switch();

        let prepared = if self.queued_video_folder == self.active_folder {
            self.queued_video.take()
        } else {
            self.clear_queued_video();
            None
        };

        self.queued_video_folder = None;
        if let Some(prepared) = prepared {
            self.start_prepared_video(prepared, true);
        } else if self.pending_single.is_none() {
            // No preloaded video ready: enqueue an async load and keep the finished frame on
            // screen until it arrives. Guarded so we enqueue only once per finished playthrough.
            self.play_next_video(ctx, true);
        }
        ctx.request_repaint();
    }

    pub(crate) fn previous_video(&mut self, ctx: &Context) {
        let Some(index) = self.history_index else {
            self.show_info("前の動画履歴がありません");
            return;
        };
        if index == 0 {
            self.show_info("前の動画履歴がありません");
            return;
        }

        let previous_index = index - 1;
        if let Some(path) = self.history.get(previous_index).cloned() {
            self.history_index = Some(previous_index);
            self.load_video(ctx, path, false);
        }
    }

    pub(crate) fn draw_player(&mut self, ctx: &Context) {
        eframe::egui::CentralPanel::default()
            .frame(Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();

                // Cap decode resolution to the panel size (physical px), bucketed to 64px so the
                // scaler does not churn during interactive resize. This lets 4K videos decode at
                // window resolution. Apply only when the bucketed value actually changes.
                let ppp = ctx.pixels_per_point();
                let target_size = bucket_target_size(rect.width() * ppp, rect.height() * ppp);
                if self.last_player_target_size != Some(target_size) {
                    self.last_player_target_size = Some(target_size);
                    if let Some(player) = self.player.as_mut() {
                        player.set_target_texture_size(target_size.0, target_size.1);
                    }
                    if let Some(prepared) = self.queued_video.as_mut() {
                        prepared
                            .player
                            .set_target_texture_size(target_size.0, target_size.1);
                    }
                }

                let mut finished = false;
                let mut should_preload = false;
                if let Some(player) = self.player.as_mut() {
                    let target = fit_rect(rect, player.size);
                    player.render_frame_at(ui, target);
                    let state_before_process = player.player_state.get();
                    player.process_state();
                    let state_after_process = player.player_state.get();

                    let elapsed_ms = player.elapsed_ms();
                    let remaining_ms = player.duration_ms.saturating_sub(elapsed_ms);
                    should_preload =
                        player.duration_ms > 0 && remaining_ms <= VIDEO_PRELOAD_BEFORE_END_MS;
                    let playback_started = player.duration_ms > 0 && elapsed_ms > 0;
                    finished = matches!(state_before_process, PlayerState::EndOfFile)
                        || matches!(state_after_process, PlayerState::EndOfFile)
                        || (matches!(state_after_process, PlayerState::Stopped)
                            && playback_started)
                        || (self.queued_video.is_some()
                            && player.duration_ms > 0
                            && remaining_ms <= VIDEO_FINISH_GRACE_MS);
                } else {
                    ui.with_layout(
                        Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            ui.label(
                                RichText::new("動画が読み込まれていません")
                                    .color(Color32::from_gray(180))
                                    .size(22.0),
                            );
                        },
                    );
                }

                if should_preload {
                    self.ensure_next_video_preloaded(ctx);
                }
                self.service_queued_video();

                if finished {
                    self.advance_after_video_finished(ctx);
                }
            });
    }
}
