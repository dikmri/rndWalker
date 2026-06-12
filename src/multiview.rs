use crate::app::{bucket_target_size, MultiViewLayout, MultiViewTile, RndWalkerApp, TileSlot};
use crate::loader::{LoadPurpose, LoadRequest};
use crate::media::choose_random_path_avoiding_recent;
use crate::playback::RANDOM_RECENT_EXCLUSION_COUNT;
use eframe::egui::{self, Color32, Context, Frame, Layout, Rect, RichText, Vec2};
use egui_video::{Player, PlayerState};
use std::path::PathBuf;

pub(crate) const MULTIVIEW_MAX_TILES: usize = 64;
pub(crate) const MULTIVIEW_RECENT_LIMIT: usize = 128;
/// Maximum number of times a single tile slot retries a failed load before giving up.
pub(crate) const MULTIVIEW_MAX_TILE_ATTEMPTS: u8 = 3;

pub(crate) fn multiview_layout_for_rect(rect: Rect, video_size: f32) -> Option<MultiViewLayout> {
    use crate::config::{MAX_MULTIVIEW_VIDEO_SIZE, MIN_MULTIVIEW_VIDEO_SIZE};

    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return None;
    }

    let target_size = video_size.clamp(MIN_MULTIVIEW_VIDEO_SIZE, MAX_MULTIVIEW_VIDEO_SIZE);
    let mut columns = (rect.width() / target_size).ceil().max(1.0) as usize;
    let mut rows = (rect.height() / target_size).ceil().max(1.0) as usize;

    while columns * rows > MULTIVIEW_MAX_TILES {
        if columns >= rows && columns > 1 {
            columns -= 1;
        } else if rows > 1 {
            rows -= 1;
        } else {
            break;
        }
    }

    Some(MultiViewLayout { columns, rows })
}

pub(crate) fn video_finished(player: &Player, before: PlayerState, after: PlayerState) -> bool {
    let playback_started = player.duration_ms > 0 && player.elapsed_ms() > 0;
    matches!(before, PlayerState::EndOfFile)
        || matches!(after, PlayerState::EndOfFile)
        || (matches!(after, PlayerState::Stopped) && playback_started)
}

pub(crate) fn render_video_cover(ui: &mut egui::Ui, player: &Player, rect: Rect) {
    ui.painter().image(
        player.texture_handle.id(),
        rect,
        cover_uv_rect(rect, player.size),
        Color32::WHITE,
    );
}

pub(crate) fn cover_uv_rect(container: Rect, content_size: Vec2) -> Rect {
    if container.width() <= 0.0
        || container.height() <= 0.0
        || content_size.x <= 0.0
        || content_size.y <= 0.0
    {
        return Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }

    let container_aspect = container.width() / container.height();
    let content_aspect = content_size.x / content_size.y;

    if content_aspect > container_aspect {
        let visible_width = (container_aspect / content_aspect).clamp(0.0, 1.0);
        let inset = (1.0 - visible_width) * 0.5;
        Rect::from_min_max(egui::pos2(inset, 0.0), egui::pos2(1.0 - inset, 1.0))
    } else {
        let visible_height = (content_aspect / container_aspect).clamp(0.0, 1.0);
        let inset = (1.0 - visible_height) * 0.5;
        Rect::from_min_max(egui::pos2(0.0, inset), egui::pos2(1.0, 1.0 - inset))
    }
}

impl RndWalkerApp {
    pub(crate) fn stop_multiview(&mut self) {
        for slot in &mut self.multiview_tiles {
            if let TileSlot::Ready(tile) = slot {
                tile.player.stop();
            }
        }
        self.multiview_tiles.clear();
        self.multiview_inflight.clear();
        self.multiview_layout = None;
        self.multiview_cell_target_size = None;
    }

    pub(crate) fn shuffle_multiview(&mut self, ctx: &Context) {
        // Generation already bumped by the caller (handle_keyboard -> shuffle path goes through
        // app.rs); bump here too to be self-contained for direct callers.
        self.bump_loader_generation();
        let Some(layout) = self.multiview_layout else {
            // No layout yet; the next draw_multiview will build from scratch.
            self.stop_multiview();
            self.show_indicator("マルチビューを再シャッフル");
            ctx.request_repaint();
            return;
        };
        self.reset_multiview_slots(layout);
        self.show_indicator("マルチビューを再シャッフル");
        ctx.request_repaint();
    }

    pub(crate) fn choose_multiview_video_path(
        &self,
        videos: &[PathBuf],
        occupied_paths: &[PathBuf],
    ) -> Option<PathBuf> {
        let mut avoided = occupied_paths.to_vec();
        avoided.extend(
            self.multiview_recent
                .iter()
                .rev()
                .take(RANDOM_RECENT_EXCLUSION_COUNT)
                .cloned(),
        );
        choose_random_path_avoiding_recent(videos, &avoided, avoided.len())
    }

    /// Paths currently displayed or being loaded for tiles other than `skip_index`.
    /// Used so a new tile load avoids duplicating what is already on screen.
    fn occupied_tile_paths(&self, skip_index: Option<usize>) -> Vec<PathBuf> {
        self.multiview_tiles
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != skip_index)
            .filter_map(|(_, slot)| match slot {
                TileSlot::Ready(tile) => Some(tile.path.clone()),
                TileSlot::Loading => None,
            })
            .chain(
                self.multiview_inflight
                    .iter()
                    .filter(|(index, _)| Some(**index) != skip_index)
                    .map(|(_, path)| path.clone()),
            )
            .collect()
    }

    /// Enqueue a `Tile` load for `index`, choosing a fresh path that avoids occupied + recent.
    /// Returns false if no playable path is available.
    fn enqueue_tile_load(&mut self, index: usize, attempts: u8) -> bool {
        let videos = self.library.active_videos(self.active_folder);
        if videos.is_empty() {
            return false;
        }
        let occupied = self.occupied_tile_paths(Some(index));
        let Some(path) = self.choose_multiview_video_path(&videos, &occupied) else {
            return false;
        };

        self.multiview_inflight.insert(index, path.clone());
        self.loader.enqueue(LoadRequest {
            generation: self.loader_generation,
            purpose: LoadPurpose::Tile { index },
            path,
            target_size: self.multiview_cell_target_size,
            economy: true,
            attempts,
        });
        true
    }

    /// Reset every slot to `Loading` and enqueue a full set of fresh tile loads.
    /// Used by shuffle and folder switch (after the generation has been bumped).
    pub(crate) fn reset_multiview_slots(&mut self, layout: MultiViewLayout) {
        for slot in &mut self.multiview_tiles {
            if let TileSlot::Ready(tile) = slot {
                tile.player.stop();
            }
        }
        self.multiview_tiles.clear();
        self.multiview_inflight.clear();
        self.multiview_layout = Some(layout);

        // With no playable videos, leave the slot list empty so the "no videos" message renders
        // instead of a wall of black cells.
        if self.library.active_videos(self.active_folder).is_empty() {
            return;
        }

        let tile_count = layout.tile_count();
        self.multiview_tiles
            .resize_with(tile_count, || TileSlot::Loading);
        for index in 0..tile_count {
            self.enqueue_tile_load(index, 0);
        }
    }

    /// Grow/shrink the slot vector to match a new layout without reshuffling existing tiles.
    fn resize_multiview_slots(&mut self, layout: MultiViewLayout) {
        let tile_count = layout.tile_count();
        let current = self.multiview_tiles.len();
        if tile_count > current {
            for index in current..tile_count {
                self.multiview_tiles.push(TileSlot::Loading);
                self.enqueue_tile_load(index, 0);
            }
        } else if tile_count < current {
            for slot in self.multiview_tiles.drain(tile_count..) {
                if let TileSlot::Ready(mut tile) = slot {
                    tile.player.stop();
                }
            }
            self.multiview_inflight
                .retain(|index, _| *index < tile_count);
        }
        self.multiview_layout = Some(layout);
    }

    /// Main-thread handler for a finished `Tile` load. On success, swaps the player into the slot
    /// (stopping the old frozen player if any). On failure, records the bad path and retries with
    /// a fresh path up to `MULTIVIEW_MAX_TILE_ATTEMPTS`.
    pub(crate) fn on_tile_loaded(
        &mut self,
        _ctx: &Context,
        index: usize,
        path: PathBuf,
        attempts: u8,
        player: Result<Player, String>,
    ) {
        self.multiview_inflight.remove(&index);
        if index >= self.multiview_tiles.len() {
            // Layout shrank after the request was issued; discard.
            if let Ok(mut player) = player {
                player.stop();
            }
            return;
        }

        match player {
            Ok(mut player) => {
                player.options.set_audio_volume(0.0);
                if let Some((width, height)) = self.multiview_cell_target_size {
                    player.set_target_texture_size(width, height);
                }
                // Stop the old frozen player (if this slot was a replacing Ready tile).
                if let TileSlot::Ready(old) = &mut self.multiview_tiles[index] {
                    old.player.stop();
                }
                self.multiview_tiles[index] = TileSlot::Ready(MultiViewTile {
                    path,
                    player,
                    replacing: false,
                });
            }
            Err(error) => {
                self.push_multiview_recent(path);
                if attempts + 1 < MULTIVIEW_MAX_TILE_ATTEMPTS {
                    if !self.enqueue_tile_load(index, attempts + 1) {
                        // No path available right now; leave the slot as-is (Loading or frozen
                        // Ready), it will retry on the next tile-finished/shuffle.
                        self.clear_tile_replacing(index);
                    }
                } else {
                    self.clear_tile_replacing(index);
                    self.show_info(format!("マルチビュー動画を読み込めません: {error}"));
                }
            }
        }
    }

    /// If the slot is a Ready tile marked as replacing, clear that flag (its replacement failed).
    fn clear_tile_replacing(&mut self, index: usize) {
        if let Some(TileSlot::Ready(tile)) = self.multiview_tiles.get_mut(index) {
            tile.replacing = false;
        }
    }

    pub(crate) fn push_multiview_recent(&mut self, path: PathBuf) {
        self.multiview_recent.push(path);
        let overflow = self
            .multiview_recent
            .len()
            .saturating_sub(MULTIVIEW_RECENT_LIMIT);
        if overflow > 0 {
            self.multiview_recent.drain(0..overflow);
        }
    }

    pub(crate) fn draw_multiview(&mut self, ctx: &Context) {
        eframe::egui::CentralPanel::default()
            .frame(Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let Some(layout) =
                    multiview_layout_for_rect(rect, self.settings.multiview_video_size)
                else {
                    return;
                };

                let cell_width = rect.width() / layout.columns as f32;
                let cell_height = rect.height() / layout.rows as f32;

                // Update the decode target size BEFORE (re)building slots, so freshly enqueued
                // tile loads already carry the right size instead of decoding at native first.
                let ppp = ctx.pixels_per_point();
                let cell_target = bucket_target_size(cell_width * ppp, cell_height * ppp);
                if self.multiview_cell_target_size != Some(cell_target) {
                    self.multiview_cell_target_size = Some(cell_target);
                    for slot in &mut self.multiview_tiles {
                        if let TileSlot::Ready(tile) = slot {
                            tile.player
                                .set_target_texture_size(cell_target.0, cell_target.1);
                        }
                    }
                }

                match self.multiview_layout {
                    None => self.reset_multiview_slots(layout),
                    Some(current) if current != layout => self.resize_multiview_slots(layout),
                    Some(_) => {}
                }

                if self.multiview_tiles.is_empty() {
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
                    return;
                }

                let mut finished_tiles = Vec::new();
                for (index, slot) in self.multiview_tiles.iter_mut().enumerate() {
                    let column = index % layout.columns;
                    let row = index / layout.columns;
                    let cell_rect = Rect::from_min_size(
                        rect.min + Vec2::new(column as f32 * cell_width, row as f32 * cell_height),
                        Vec2::new(cell_width, cell_height),
                    );

                    match slot {
                        TileSlot::Loading => {
                            // Plain black cell while the first player loads.
                            ui.painter().rect_filled(cell_rect, 0.0, Color32::BLACK);
                        }
                        TileSlot::Ready(tile) => {
                            render_video_cover(ui, &tile.player, cell_rect);
                            let before = tile.player.player_state.get();
                            tile.player.process_state();
                            let after = tile.player.player_state.get();
                            // Keep rendering the frozen last frame while a replacement loads; only
                            // enqueue a replacement once per finished playthrough.
                            if !tile.replacing && video_finished(&tile.player, before, after) {
                                finished_tiles.push((index, tile.path.clone()));
                                tile.replacing = true;
                            }
                        }
                    }
                }

                for (index, old_path) in finished_tiles {
                    self.push_multiview_recent(old_path);
                    if !self.enqueue_tile_load(index, 0) {
                        // No fresh path available: clear the guard so it retries next finish.
                        self.clear_tile_replacing(index);
                    }
                }
            });
    }
}
