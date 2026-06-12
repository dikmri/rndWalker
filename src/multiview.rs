use crate::app::{bucket_target_size, MultiViewTile, RndWalkerApp, TileSlot};
use crate::loader::{LoadPurpose, LoadRequest};
use crate::media::choose_random_path_avoiding_recent;
use crate::playback::RANDOM_RECENT_EXCLUSION_COUNT;
use eframe::egui::{self, pos2, Color32, Context, Frame, Layout, Rect, RichText};
use egui_video::{Player, PlayerState};
use std::path::PathBuf;

pub(crate) const MULTIVIEW_MAX_TILES: usize = 64;
pub(crate) const MULTIVIEW_RECENT_LIMIT: usize = 128;
/// Maximum number of times a single tile slot retries a failed load before giving up.
pub(crate) const MULTIVIEW_MAX_TILE_ATTEMPTS: u8 = 3;
/// Begin loading a tile's replacement this many ms before its current video ends, so the new
/// player is ready to swap in the instant the old one finishes (no frozen-frame wait).
const TILE_PRELOAD_BEFORE_END_MS: i64 = 3_000;

/// Fallback aspect ratio used for tiles whose real size is not yet known (still loading) or
/// invalid (non-positive dimensions).
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// One rect per input aspect, in order. Rows fill the width exactly at their natural height
/// (width / sum(aspects)); rows stack from the top and the last rows may extend past
/// `area.bottom()` — callers rely on clipping. No vertical scaling: aspect ratios are exact.
///
/// Each complete row is built greedily in input order: tiles are appended until their combined
/// aspect sum would fill the width at `target_row_height`, then the row height is set to
/// `area.width() / sum(aspects)` so the unchanged aspect ratios pack to the full width and the
/// last tile of the row ends exactly at `area.right()`. A trailing INCOMPLETE row (tiles ran out
/// before filling the width) is laid out at `target_row_height` with natural widths
/// (`aspect * target_row_height`); it does not stretch to the right edge. Rows whose top falls
/// below `area.bottom()` still receive rects; the egui painter clips them.
///
/// Videos are never cropped: every tile shows its full frame at its row height.
pub(crate) fn justified_layout(area: Rect, target_row_height: f32, aspects: &[f32]) -> Vec<Rect> {
    // Guard: nothing to place, or a degenerate area.
    if aspects.is_empty() || area.width() <= 1.0 || area.height() <= 1.0 {
        return Vec::new();
    }

    let target_row_height = target_row_height.max(1.0);
    let width = area.width();

    let mut rects: Vec<Rect> = Vec::with_capacity(aspects.len());
    let mut y = area.top();

    // Greedy row assignment in input order.
    let mut row_start = 0usize;
    let mut aspect_sum = 0.0f32;
    let mut index = 0usize;
    while index < aspects.len() {
        let aspect = sanitize_aspect(aspects[index]);
        aspect_sum += aspect;
        index += 1;

        // The row is complete once it would fill the width at the target height.
        if aspect_sum * target_row_height >= width {
            let row_height = width / aspect_sum;
            let mut x = area.left();
            for offset in 0..(index - row_start) {
                let tile_aspect = sanitize_aspect(aspects[row_start + offset]);
                let is_last_in_row = offset + 1 == index - row_start;
                // Pin the last tile exactly to the right edge to absorb float error.
                let right = if is_last_in_row {
                    area.right()
                } else {
                    x + tile_aspect * row_height
                };
                rects.push(Rect::from_min_max(pos2(x, y), pos2(right, y + row_height)));
                x = right;
            }
            y += row_height;
            row_start = index;
            aspect_sum = 0.0;
        }
    }

    // Trailing incomplete row: natural widths at the target height, no stretch to the right edge.
    if row_start < index {
        let mut x = area.left();
        for offset in 0..(index - row_start) {
            let tile_aspect = sanitize_aspect(aspects[row_start + offset]);
            let right = x + tile_aspect * target_row_height;
            rects.push(Rect::from_min_max(
                pos2(x, y),
                pos2(right, y + target_row_height),
            ));
            x = right;
        }
    }

    rects
}

/// How many tiles should exist to fill `area` (with one row of overflow at the bottom), given the
/// row height and the average tile aspect. Cheap to recompute every frame.
///
/// `per_row` and `row_count` are derived independently; the `+1` row guarantees the bottom always
/// overflows (and is clipped) rather than underfilling. Result is clamped to a sane tile budget.
fn desired_tile_count(area: Rect, row_height: f32, avg_aspect: f32) -> usize {
    let row_height = row_height.max(1.0);
    let avg_aspect = sanitize_aspect(avg_aspect);
    let per_row = ((area.width() / (row_height * avg_aspect)).ceil() as usize).max(1);
    let row_count = (((area.height() / row_height).ceil() as usize) + 1).max(1);
    (per_row * row_count).clamp(1, MULTIVIEW_MAX_TILES)
}

/// Number of tiles per row for the hysteresis threshold, mirroring `desired_tile_count`'s `per_row`.
fn tiles_per_row(area: Rect, row_height: f32, avg_aspect: f32) -> usize {
    let row_height = row_height.max(1.0);
    let avg_aspect = sanitize_aspect(avg_aspect);
    ((area.width() / (row_height * avg_aspect)).ceil() as usize).max(1)
}

/// Clamp an aspect ratio to a sane positive range, substituting the fallback for invalid values.
fn sanitize_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect.clamp(0.1, 10.0)
    } else {
        FALLBACK_ASPECT
    }
}

pub(crate) fn video_finished(player: &Player, before: PlayerState, after: PlayerState) -> bool {
    let playback_started = player.duration_ms > 0 && player.elapsed_ms() > 0;
    matches!(before, PlayerState::EndOfFile)
        || matches!(after, PlayerState::EndOfFile)
        || (matches!(after, PlayerState::Stopped) && playback_started)
}

/// Whether a Ready tile's player has already reached the end of its video (it is rendering a
/// frozen last frame). Used so a freshly arrived replacement knows whether to swap in immediately
/// (the old video already finished) or stash itself as a preload (the old video is still playing).
fn tile_player_finished(player: &Player) -> bool {
    let playback_started = player.duration_ms > 0 && player.elapsed_ms() > 0;
    matches!(player.player_state.get(), PlayerState::EndOfFile)
        || (matches!(player.player_state.get(), PlayerState::Stopped) && playback_started)
}

impl RndWalkerApp {
    pub(crate) fn stop_multiview(&mut self) {
        for slot in self.multiview_tiles.drain(..) {
            if let TileSlot::Ready(tile) = slot {
                self.loader.dispose(tile.player);
                if let Some((_, next_player)) = tile.next {
                    self.loader.dispose(next_player);
                }
            }
        }
        self.multiview_inflight.clear();
    }

    pub(crate) fn shuffle_multiview(&mut self, ctx: &Context) {
        // Bump the generation so in-flight loads are discarded, then drop all slots. The next
        // draw_multiview rebuilds a fresh set sized to the current window.
        self.bump_loader_generation();
        self.stop_multiview();
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

    /// Paths currently displayed, stashed as a pending replacement, or being loaded for tiles
    /// other than `skip_index`. Used so a new tile load avoids duplicating what is already present.
    ///
    /// Pass `None` for `skip_index` to count every slot including the one being replaced; this is
    /// used for replacement/preload picks so a tile does not immediately replay its own video.
    fn occupied_tile_paths(&self, skip_index: Option<usize>) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for (index, slot) in self.multiview_tiles.iter().enumerate() {
            if Some(index) == skip_index {
                continue;
            }
            if let TileSlot::Ready(tile) = slot {
                paths.push(tile.path.clone());
                // A stashed replacement is already spoken for; other picks must avoid it too.
                if let Some((next_path, _)) = &tile.next {
                    paths.push(next_path.clone());
                }
            }
        }
        for (index, path) in &self.multiview_inflight {
            if Some(*index) != skip_index {
                paths.push(path.clone());
            }
        }
        paths
    }

    /// Bucketed decode size estimate for a freshly enqueued tile, based on the configured row
    /// height. The per-frame pass refines this to each tile's actual rect once it is placed.
    fn tile_load_target_size(&self, ppp: f32) -> (u32, u32) {
        let row_height = self.clamped_multiview_row_height();
        bucket_target_size(row_height * FALLBACK_ASPECT * ppp, row_height * ppp)
    }

    fn clamped_multiview_row_height(&self) -> f32 {
        use crate::config::{MAX_MULTIVIEW_VIDEO_SIZE, MIN_MULTIVIEW_VIDEO_SIZE};
        self.settings
            .multiview_video_size
            .clamp(MIN_MULTIVIEW_VIDEO_SIZE, MAX_MULTIVIEW_VIDEO_SIZE)
    }

    /// Enqueue a `Tile` load for `index`, choosing a fresh path that avoids occupied + recent.
    /// Returns false if no playable path is available.
    fn enqueue_tile_load(&mut self, index: usize, attempts: u8, ppp: f32) -> bool {
        let videos = self.library.active_videos(self.active_folder);
        if videos.is_empty() {
            return false;
        }
        // Avoid every occupied slot, including this one's own current path, so a finishing tile
        // does not immediately replay the same video.
        let occupied = self.occupied_tile_paths(None);
        let Some(path) = self.choose_multiview_video_path(&videos, &occupied) else {
            return false;
        };

        self.multiview_inflight.insert(index, path.clone());
        self.loader.enqueue(LoadRequest {
            generation: self.loader_generation,
            purpose: LoadPurpose::Tile { index },
            path,
            target_size: Some(self.tile_load_target_size(ppp)),
            economy: true,
            attempts,
        });
        true
    }

    /// Build an initial set of `Loading` slots sized to roughly fill the current window, then
    /// enqueue a fresh tile load for each. The per-frame adaptation converges to the exact count.
    fn build_initial_multiview_slots(&mut self, area: Rect, ppp: f32) {
        // Dispose any existing players (and stashed replacements) off the UI thread.
        self.stop_multiview();

        // With no playable videos, leave the slot list empty so the "no videos" message renders
        // instead of a wall of black cells.
        if self.library.active_videos(self.active_folder).is_empty() {
            return;
        }

        // Same formula as draw_multiview's desired_count, with FALLBACK_ASPECT for all tiles
        // (none are Ready yet).
        let row_height = self.clamped_multiview_row_height();
        let tile_count = desired_tile_count(area, row_height, FALLBACK_ASPECT);

        self.multiview_tiles
            .resize_with(tile_count, || TileSlot::Loading);
        for index in 0..tile_count {
            self.enqueue_tile_load(index, 0, ppp);
        }
    }

    /// Main-thread handler for a finished `Tile` load.
    ///
    /// On success, the action depends on the current slot:
    /// - `Loading` (initial / deficit fill): install and play immediately.
    /// - `Ready` whose video already finished (it was frozen, waiting): swap immediately, disposing
    ///   the old player off the UI thread.
    /// - `Ready` still playing (this was a preload): pause the new player and stash it in `next`,
    ///   keeping `next_requested = true` so the preload trigger does not refire.
    ///
    /// On failure, records the bad path and retries with a fresh path up to
    /// `MULTIVIEW_MAX_TILE_ATTEMPTS`; on final failure clears `next_requested` so a later finish
    /// can retry.
    pub(crate) fn on_tile_loaded(
        &mut self,
        ctx: &Context,
        index: usize,
        path: PathBuf,
        attempts: u8,
        player: Result<Player, String>,
    ) {
        self.multiview_inflight.remove(&index);
        if index >= self.multiview_tiles.len() {
            // Slot vector shrank after the request was issued; discard off the UI thread.
            if let Ok(player) = player {
                self.loader.dispose(player);
            }
            return;
        }

        let ppp = ctx.pixels_per_point();
        match player {
            Ok(mut player) => {
                player.options.set_audio_volume(0.0);
                // Seed a reasonable decode size; the per-frame pass corrects it to the real rect.
                let target = self.tile_load_target_size(ppp);
                player.set_target_texture_size(target.0, target.1);

                match std::mem::replace(&mut self.multiview_tiles[index], TileSlot::Loading) {
                    TileSlot::Loading => {
                        // Fresh slot: install and play (the worker already called start()).
                        self.multiview_tiles[index] = TileSlot::Ready(MultiViewTile {
                            path,
                            player,
                            next: None,
                            next_requested: false,
                            last_target: Some(target),
                        });
                    }
                    TileSlot::Ready(mut old) => {
                        if tile_player_finished(&old.player) {
                            // Old video already finished and is frozen: swap in now. The old path
                            // was already pushed to recent when it finished, so do not re-push.
                            self.loader.dispose(old.player);
                            // Defensive: if a stale `next` somehow lingered, dispose it too.
                            if let Some((_, stale)) = old.next.take() {
                                self.loader.dispose(stale);
                            }
                            self.multiview_tiles[index] = TileSlot::Ready(MultiViewTile {
                                path,
                                player,
                                next: None,
                                next_requested: false,
                                last_target: Some(target),
                            });
                        } else {
                            // Preload arrived while the old video is still playing: pause it (caps
                            // how far ahead it runs) and stash it. Keep next_requested = true.
                            player.pause();
                            if let Some((_, stale)) = old.next.replace((path, player)) {
                                // Should not happen given the invariant; dispose defensively.
                                self.loader.dispose(stale);
                            }
                            old.next_requested = true;
                            self.multiview_tiles[index] = TileSlot::Ready(old);
                        }
                    }
                }
            }
            Err(error) => {
                self.push_multiview_recent(path);
                if attempts + 1 < MULTIVIEW_MAX_TILE_ATTEMPTS {
                    if !self.enqueue_tile_load(index, attempts + 1, ppp) {
                        // No path available right now; leave the slot as-is (Loading or frozen
                        // Ready), it will retry on the next tile-finished/shuffle.
                        self.clear_tile_next_requested(index);
                    }
                } else {
                    self.clear_tile_next_requested(index);
                    self.show_info(format!("マルチビュー動画を読み込めません: {error}"));
                }
            }
        }
    }

    /// If the slot is a Ready tile, clear its `next_requested` flag (its replacement load failed),
    /// so a later finish can retry.
    fn clear_tile_next_requested(&mut self, index: usize) {
        if let Some(TileSlot::Ready(tile)) = self.multiview_tiles.get_mut(index) {
            tile.next_requested = false;
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
                let area = ui.max_rect();
                if area.width() <= 1.0 || area.height() <= 1.0 {
                    return;
                }
                let ppp = ctx.pixels_per_point();

                // Rebuild from scratch when there are no slots but the active library has videos.
                // Window resizes need no rebuild: the per-frame reflow handles them.
                if self.multiview_tiles.is_empty()
                    && !self.library.active_videos(self.active_folder).is_empty()
                {
                    self.build_initial_multiview_slots(area, ppp);
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

                // Aspect per slot: real size for Ready tiles, fallback for Loading ones.
                let aspects: Vec<f32> = self
                    .multiview_tiles
                    .iter()
                    .map(|slot| match slot {
                        TileSlot::Ready(tile) => {
                            let size = tile.player.size;
                            if size.x > 0.0 && size.y > 0.0 {
                                size.x / size.y
                            } else {
                                FALLBACK_ASPECT
                            }
                        }
                        TileSlot::Loading => FALLBACK_ASPECT,
                    })
                    .collect();

                let row_height = self.clamped_multiview_row_height();
                let rects = justified_layout(area, row_height, &aspects);

                // Deferred actions: we cannot borrow `self.loader` / call `self` methods while
                // iterating `self.multiview_tiles` mutably, so collect and apply after the loop.
                let mut to_dispose: Vec<Player> = Vec::new();
                let mut recent_to_push: Vec<PathBuf> = Vec::new();
                let mut to_enqueue: Vec<usize> = Vec::new();
                let preload_target = self.tile_load_target_size(ppp);

                for (index, slot) in self.multiview_tiles.iter_mut().enumerate() {
                    let rect = rects.get(index).copied();
                    match slot {
                        TileSlot::Loading => {
                            if let Some(rect) = rect {
                                ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
                            }
                        }
                        TileSlot::Ready(tile) => {
                            if let Some(rect) = rect {
                                // Match the decode size to this tile's actual on-screen rect,
                                // only re-targeting the scaler when the bucketed value changes.
                                let target =
                                    bucket_target_size(rect.width() * ppp, rect.height() * ppp);
                                if tile.last_target != Some(target) {
                                    tile.player.set_target_texture_size(target.0, target.1);
                                    tile.last_target = Some(target);
                                }
                                // Draw the FULL texture; no cropping.
                                ui.painter().image(
                                    tile.player.texture_handle.id(),
                                    rect,
                                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            let before = tile.player.player_state.get();
                            tile.player.process_state();
                            let after = tile.player.player_state.get();

                            // Preload trigger: a few seconds before the end, start loading the
                            // replacement so it is ready to swap in the instant this video ends.
                            let elapsed_ms = tile.player.elapsed_ms();
                            let remaining_ms = tile.player.duration_ms.saturating_sub(elapsed_ms);
                            if tile.player.duration_ms > 0
                                && remaining_ms <= TILE_PRELOAD_BEFORE_END_MS
                                && tile.next.is_none()
                                && !tile.next_requested
                            {
                                tile.next_requested = true;
                                // Record the outgoing path now (the single point where a
                                // replacement is first requested), so it is in `recent` no matter
                                // which path later performs the swap.
                                recent_to_push.push(tile.path.clone());
                                to_enqueue.push(index);
                            }

                            if video_finished(&tile.player, before, after) {
                                if let Some((next_path, mut next_player)) = tile.next.take() {
                                    // Replacement preloaded: swap it in instantly. The old player
                                    // goes off-thread for disposal; its path already entered
                                    // recent when the replacement was requested.
                                    next_player.resume();
                                    next_player.set_target_texture_size(
                                        preload_target.0,
                                        preload_target.1,
                                    );
                                    let old = std::mem::replace(&mut tile.player, next_player);
                                    to_dispose.push(old);
                                    tile.path = next_path;
                                    tile.next_requested = false;
                                    tile.last_target = Some(preload_target);
                                } else if !tile.next_requested {
                                    // No preload in flight (it never triggered, or a prior load
                                    // failed): keep the frozen frame and request a replacement now.
                                    tile.next_requested = true;
                                    recent_to_push.push(tile.path.clone());
                                    to_enqueue.push(index);
                                }
                                // else: a preload is still in flight; keep the frozen last frame.
                                // on_tile_loaded will swap it in when it arrives.
                            }
                        }
                    }
                }

                for old in to_dispose {
                    self.loader.dispose(old);
                }
                for path in recent_to_push {
                    self.push_multiview_recent(path);
                }
                for index in to_enqueue {
                    if !self.enqueue_tile_load(index, 0, ppp) {
                        // No fresh path available: clear the guard so it retries next finish.
                        self.clear_tile_next_requested(index);
                    }
                }

                // Tile-count management. Recompute the desired count each frame from the current
                // area and the average aspect of Ready tiles, then only act when off by at least a
                // full row (hysteresis). Acting on the whole deficit/surplus at once — instead of
                // one tile per frame — avoids the add/remove oscillation that flooded the loader.
                let mut aspect_sum = 0.0f32;
                let mut ready_count = 0usize;
                for slot in &self.multiview_tiles {
                    if let TileSlot::Ready(tile) = slot {
                        let size = tile.player.size;
                        if size.x > 0.0 && size.y > 0.0 {
                            aspect_sum += size.x / size.y;
                            ready_count += 1;
                        }
                    }
                }
                let avg_aspect = if ready_count > 0 {
                    aspect_sum / ready_count as f32
                } else {
                    FALLBACK_ASPECT
                };

                let len = self.multiview_tiles.len();
                let desired = desired_tile_count(area, row_height, avg_aspect);
                let per_row = tiles_per_row(area, row_height, avg_aspect);
                let has_videos = !self.library.active_videos(self.active_folder).is_empty();

                if has_videos && len.abs_diff(desired) >= per_row {
                    if desired > len {
                        // Deficit: add the whole shortfall as Loading slots and enqueue each.
                        for index in len..desired {
                            self.multiview_tiles.push(TileSlot::Loading);
                            self.enqueue_tile_load(index, 0, ppp);
                        }
                    } else {
                        // Surplus: pop the whole excess from the end, disposing Ready players (and
                        // any stashed replacement) off the UI thread and dropping any in-flight
                        // load tracked for those indices.
                        for index in (desired..len).rev() {
                            self.multiview_inflight.remove(&index);
                            if let Some(TileSlot::Ready(tile)) = self.multiview_tiles.pop() {
                                self.loader.dispose(tile.player);
                                if let Some((_, next_player)) = tile.next {
                                    self.loader.dispose(next_player);
                                }
                            }
                        }
                    }
                    ctx.request_repaint();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::justified_layout;
    use eframe::egui::{pos2, Rect};

    fn area(width: f32, height: f32) -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(width, height))
    }

    /// Group rects into rows by their shared top edge (input order is preserved).
    fn group_rows(rects: &[Rect]) -> Vec<Vec<Rect>> {
        let mut rows: Vec<Vec<Rect>> = Vec::new();
        for &rect in rects {
            match rows.last_mut() {
                Some(row) if (row[0].top() - rect.top()).abs() < 0.5 => row.push(rect),
                _ => rows.push(vec![rect]),
            }
        }
        rows
    }

    /// A row is "complete" when its tile widths span the full area width (last tile pinned right).
    fn is_complete_row(row: &[Rect], a: Rect) -> bool {
        (row.last().unwrap().right() - a.right()).abs() < 0.01
    }

    /// Every complete row's tile widths sum exactly to the area width.
    #[test]
    fn complete_rows_fill_width_exactly() {
        let aspects = vec![16.0 / 9.0; 12];
        let a = area(1600.0, 900.0);
        let rects = justified_layout(a, 300.0, &aspects);
        assert!(!rects.is_empty());

        let rows = group_rows(&rects);
        for row in &rows {
            if !is_complete_row(row, a) {
                continue; // trailing incomplete row.
            }
            assert!(
                (row.first().unwrap().left() - a.left()).abs() < 0.01,
                "row starts at left"
            );
            assert!(
                (row.last().unwrap().right() - a.right()).abs() < 0.01,
                "row ends at right"
            );
        }
    }

    /// Placed rects must not overlap and must stack contiguously from the top.
    #[test]
    fn rects_tile_without_gaps_or_overlap() {
        let aspects = vec![
            16.0 / 9.0,
            9.0 / 16.0,
            1.0,
            16.0 / 9.0,
            1.0,
            9.0 / 16.0,
            1.5,
            1.2,
        ];
        let a = area(1280.0, 720.0);
        let rects = justified_layout(a, 240.0, &aspects);
        assert!(!rects.is_empty());

        let rows = group_rows(&rects);
        // Within a row, each tile's left equals the previous tile's right.
        for row in &rows {
            for pair in row.windows(2) {
                assert!(
                    (pair[0].right() - pair[1].left()).abs() < 0.01,
                    "no horizontal gap/overlap within a row"
                );
            }
        }
        // Rows stack contiguously: each row's bottom equals the next row's top.
        for pair in rows.windows(2) {
            assert!(
                (pair[0][0].bottom() - pair[1][0].top()).abs() < 0.01,
                "no vertical gap/overlap between rows"
            );
        }
        // First row touches the top.
        assert!((rows.first().unwrap()[0].top() - a.top()).abs() < 0.01);
    }

    /// Every input aspect gets a rect, in order.
    #[test]
    fn every_input_gets_a_rect() {
        let aspects = vec![16.0 / 9.0, 1.0, 9.0 / 16.0, 1.5, 2.0, 1.2, 16.0 / 9.0];
        let a = area(1000.0, 600.0);
        let rects = justified_layout(a, 200.0, &aspects);
        assert_eq!(rects.len(), aspects.len());
    }

    /// Each rect in a complete row preserves its input aspect exactly (no distortion).
    #[test]
    fn complete_row_rects_preserve_aspect_exactly() {
        let aspects = vec![16.0 / 9.0, 1.0, 9.0 / 16.0, 1.5, 2.0, 1.2, 16.0 / 9.0, 1.0];
        let a = area(1280.0, 720.0);
        let rects = justified_layout(a, 240.0, &aspects);

        let rows = group_rows(&rects);
        let mut input_index = 0usize;
        for row in &rows {
            let complete = is_complete_row(row, a);
            for rect in row {
                if complete {
                    let drawn = rect.width() / rect.height();
                    let expected = aspects[input_index];
                    assert!(
                        (drawn / expected - 1.0).abs() < 1e-3,
                        "tile {input_index}: drawn {drawn} vs expected {expected}"
                    );
                }
                input_index += 1;
            }
        }
    }

    /// A trailing incomplete row uses height == target_row_height with natural widths.
    #[test]
    fn incomplete_row_uses_target_height() {
        // One short 16:9 tile in a wide area cannot fill the width at 200px height.
        let aspects = vec![16.0 / 9.0];
        let a = area(1600.0, 900.0);
        let target = 200.0;
        let rects = justified_layout(a, target, &aspects);
        assert_eq!(rects.len(), 1);
        let rect = rects[0];
        assert!(
            (rect.height() - target).abs() < 0.01,
            "incomplete row height {} should equal target {target}",
            rect.height()
        );
        // Natural width, not stretched to the right edge.
        assert!(rect.right() < a.right());
        assert!((rect.width() / rect.height() - 16.0 / 9.0).abs() < 1e-3);
    }

    /// With enough tiles to fill the height, the last row's bottom overflows past area.bottom().
    #[test]
    fn filled_layout_overflows_bottom() {
        let aspects = vec![16.0 / 9.0; 40];
        let a = area(1600.0, 900.0);
        let rects = justified_layout(a, 300.0, &aspects);
        let bottom = rects.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
        assert!(
            bottom >= a.bottom() - 0.01,
            "bottom {bottom} should reach/overflow area bottom {}",
            a.bottom()
        );
    }

    /// Empty input or a degenerate area yields no rects.
    #[test]
    fn empty_inputs_yield_empty_layout() {
        assert!(justified_layout(area(1600.0, 900.0), 300.0, &[]).is_empty());
        assert!(justified_layout(area(0.5, 0.5), 300.0, &[16.0 / 9.0; 4]).is_empty());
    }
}
