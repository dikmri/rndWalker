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

/// Fallback aspect ratio used for tiles whose real size is not yet known (still loading) or
/// invalid (non-positive dimensions).
const FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Result of [`justified_layout`]: a tightly packed, aspect-preserving "masonry" tiling.
pub(crate) struct JustifiedLayout {
    /// One rect per tile index in input order; tiles beyond `used_tiles` get no rect.
    pub rects: Vec<Rect>,
    /// Number of leading tiles actually placed.
    pub used_tiles: usize,
    /// True if adding more tiles would meaningfully improve vertical fill.
    pub needs_more: bool,
}

/// Lay out `aspects` (width / height per tile) into `area` as justified rows of `target_row_height`.
///
/// Each row fills `area` horizontally exactly: tiles are appended greedily until their combined
/// aspect sum would overflow the width, then the row height is set to `area.width() / sum(aspects)`
/// so the unchanged aspect ratios pack to the full width. Rows stack until the cumulative height
/// reaches `area.height()`. A single scale factor (clamped to [0.75, 1.33]) is applied to row
/// heights to fit the area vertically; widths are unaffected, so the horizontal fill stays exact
/// and the only distortion is that vertical scale factor (a few percent in practice).
///
/// Videos are never cropped: every tile shows its full frame at the row's (possibly scaled) height.
pub(crate) fn justified_layout(
    area: Rect,
    target_row_height: f32,
    aspects: &[f32],
) -> JustifiedLayout {
    // Guard: nothing to place, or a degenerate area.
    if aspects.is_empty() || area.width() <= 1.0 || area.height() <= 1.0 {
        return JustifiedLayout {
            rects: Vec::new(),
            used_tiles: 0,
            needs_more: false,
        };
    }

    let target_row_height = target_row_height.max(1.0);
    let width = area.width();

    // Greedy row assignment in input order. Each entry is (start_index, len, row_height).
    struct Row {
        start: usize,
        len: usize,
        height: f32,
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut cumulative_height = 0.0f32;

    let mut row_start = 0usize;
    let mut aspect_sum = 0.0f32;
    let mut index = 0usize;
    while index < aspects.len() {
        let aspect = sanitize_aspect(aspects[index]);
        aspect_sum += aspect;
        index += 1;

        // The row is complete once it would fill the width at the target height.
        if aspect_sum * target_row_height >= width {
            let height = width / aspect_sum;
            rows.push(Row {
                start: row_start,
                len: index - row_start,
                height,
            });
            cumulative_height += height;
            row_start = index;
            aspect_sum = 0.0;

            // Stop once the rows (including the overshoot row just added) fill the height.
            if cumulative_height >= area.height() {
                break;
            }
        }
    }

    // Trailing incomplete row: also fill the width exactly at its own (taller) height.
    if row_start < index && aspect_sum > 0.0 {
        let height = width / aspect_sum;
        rows.push(Row {
            start: row_start,
            len: index - row_start,
            height,
        });
        cumulative_height += height;
    }

    if rows.is_empty() {
        return JustifiedLayout {
            rects: Vec::new(),
            used_tiles: 0,
            needs_more: false,
        };
    }

    // Vertical fit. Candidate (a): keep all rows, squeeze by f = height / total (<= 1).
    // Candidate (b): drop the last row, stretch by f' = height / (total - h_last) (>= 1).
    let total = cumulative_height;
    let last_height = rows.last().map(|row| row.height).unwrap_or(0.0);
    let factor_all = area.height() / total;
    let factor_drop_last = if rows.len() > 1 && (total - last_height) > 0.0 {
        Some(area.height() / (total - last_height))
    } else {
        None
    };

    let drop_last = match factor_drop_last {
        Some(drop_factor) => (drop_factor - 1.0).abs() < (factor_all - 1.0).abs(),
        None => false,
    };

    let (best_factor, place_rows) = if drop_last {
        (factor_drop_last.unwrap(), rows.len() - 1)
    } else {
        (factor_all, rows.len())
    };

    // `needs_more` reflects real undershoot: tiles ran out (we never broke early on height) AND the
    // unclamped best factor would have to stretch beyond 1.15 to fill the area.
    let tiles_exhausted = index >= aspects.len() && cumulative_height < area.height();
    let needs_more = tiles_exhausted && best_factor > 1.15;

    let factor = best_factor.clamp(0.75, 1.33);
    // When the factor had to be clamped (severe under/overshoot, e.g. while tiles are still
    // loading), do NOT pin the last row to the bottom edge — that would stretch it wildly.
    let factor_clamped = factor != best_factor;

    // Emit rects row by row. Widths use the UNSCALED row height so each row still fills the
    // width exactly and the vertical-fit distortion spreads evenly over all tiles instead of
    // accumulating in the last tile of each row. The last tile of each row ends exactly at
    // area.right(); when the rows fill the height, the final row ends exactly at area.bottom().
    let mut rects: Vec<Rect> = Vec::new();
    let mut y = area.top();
    for (row_index, row) in rows.iter().take(place_rows).enumerate() {
        let row_height = row.height * factor;
        let is_last_row = row_index + 1 == place_rows;
        let row_bottom = if is_last_row && !factor_clamped {
            area.bottom()
        } else {
            y + row_height
        };

        let mut x = area.left();
        for offset in 0..row.len {
            let tile_index = row.start + offset;
            let aspect = sanitize_aspect(aspects[tile_index]);
            let is_last_in_row = offset + 1 == row.len;
            let right = if is_last_in_row {
                area.right()
            } else {
                x + aspect * row.height
            };
            rects.push(Rect::from_min_max(pos2(x, y), pos2(right, row_bottom)));
            x = right;
        }
        y = row_bottom;
    }

    let used_tiles = rects.len();
    JustifiedLayout {
        rects,
        used_tiles,
        needs_more,
    }
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

impl RndWalkerApp {
    pub(crate) fn stop_multiview(&mut self) {
        for slot in &mut self.multiview_tiles {
            if let TileSlot::Ready(tile) = slot {
                tile.player.stop();
            }
        }
        self.multiview_tiles.clear();
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
        let occupied = self.occupied_tile_paths(Some(index));
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
        for slot in &mut self.multiview_tiles {
            if let TileSlot::Ready(tile) = slot {
                tile.player.stop();
            }
        }
        self.multiview_tiles.clear();
        self.multiview_inflight.clear();

        // With no playable videos, leave the slot list empty so the "no videos" message renders
        // instead of a wall of black cells.
        if self.library.active_videos(self.active_folder).is_empty() {
            return;
        }

        let row_height = self.clamped_multiview_row_height();
        let cols = (area.width() / (row_height * FALLBACK_ASPECT))
            .ceil()
            .max(1.0) as usize;
        let row_count = (area.height() / row_height).ceil().max(1.0) as usize;
        let tile_count = (cols * row_count).clamp(1, MULTIVIEW_MAX_TILES);

        self.multiview_tiles
            .resize_with(tile_count, || TileSlot::Loading);
        for index in 0..tile_count {
            self.enqueue_tile_load(index, 0, ppp);
        }
    }

    /// Main-thread handler for a finished `Tile` load. On success, swaps the player into the slot
    /// (stopping the old frozen player if any). On failure, records the bad path and retries with
    /// a fresh path up to `MULTIVIEW_MAX_TILE_ATTEMPTS`.
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
            // Slot vector shrank after the request was issued; discard.
            if let Ok(mut player) = player {
                player.stop();
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
                // Stop the old frozen player (if this slot was a replacing Ready tile).
                if let TileSlot::Ready(old) = &mut self.multiview_tiles[index] {
                    old.player.stop();
                }
                self.multiview_tiles[index] = TileSlot::Ready(MultiViewTile {
                    path,
                    player,
                    replacing: false,
                    last_target: Some(target),
                });
            }
            Err(error) => {
                self.push_multiview_recent(path);
                if attempts + 1 < MULTIVIEW_MAX_TILE_ATTEMPTS {
                    if !self.enqueue_tile_load(index, attempts + 1, ppp) {
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
                let layout = justified_layout(area, row_height, &aspects);

                let mut finished_tiles = Vec::new();
                for (index, slot) in self.multiview_tiles.iter_mut().enumerate() {
                    let rect = layout.rects.get(index).copied();
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
                    if !self.enqueue_tile_load(index, 0, ppp) {
                        // No fresh path available: clear the guard so it retries next finish.
                        self.clear_tile_replacing(index);
                    }
                }

                // At most one corrective tile-count action per frame, to avoid thrash.
                let len = self.multiview_tiles.len();
                if layout.needs_more
                    && len < MULTIVIEW_MAX_TILES
                    && !self.library.active_videos(self.active_folder).is_empty()
                {
                    let index = len;
                    self.multiview_tiles.push(TileSlot::Loading);
                    self.enqueue_tile_load(index, 0, ppp);
                    ctx.request_repaint();
                } else if layout.used_tiles < len {
                    // Surplus: drop one slot from the end.
                    if let Some(TileSlot::Ready(mut tile)) = self.multiview_tiles.pop() {
                        tile.player.stop();
                    }
                    self.multiview_inflight.remove(&(len - 1));
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

    /// Every row's tile widths must sum exactly to the area width.
    #[test]
    fn rows_fill_width_exactly() {
        let aspects = vec![16.0 / 9.0; 12];
        let a = area(1600.0, 900.0);
        let layout = justified_layout(a, 300.0, &aspects);
        assert!(layout.used_tiles > 0);

        // Group rects by their top edge to detect rows.
        let mut rows: Vec<Vec<Rect>> = Vec::new();
        for &rect in &layout.rects {
            match rows.last_mut() {
                Some(row) if (row[0].top() - rect.top()).abs() < 0.5 => row.push(rect),
                _ => rows.push(vec![rect]),
            }
        }
        for row in &rows {
            let left = row.first().unwrap().left();
            let right = row.last().unwrap().right();
            assert!((left - a.left()).abs() < 0.01, "row starts at left");
            assert!(
                (right - a.right()).abs() < 0.01,
                "row ends at right: {right}"
            );
        }
    }

    /// Placed rects must not overlap and must tile the area top-to-bottom without gaps.
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
        let layout = justified_layout(a, 240.0, &aspects);
        assert!(layout.used_tiles > 0);

        // Within a row, each tile's left equals the previous tile's right.
        let mut rows: Vec<Vec<Rect>> = Vec::new();
        for &rect in &layout.rects {
            match rows.last_mut() {
                Some(row) if (row[0].top() - rect.top()).abs() < 0.5 => row.push(rect),
                _ => rows.push(vec![rect]),
            }
        }
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

    /// Uniform 16:9 tiles in a 16:9 area should need almost no vertical scaling.
    #[test]
    fn uniform_tiles_in_matching_area_have_near_one_factor() {
        // 16:9 area; choose enough tiles to make a clean grid.
        let aspects = vec![16.0 / 9.0; 12];
        let a = area(1920.0, 1080.0);
        let target = 360.0; // 1080 / 3 rows.
        let layout = justified_layout(a, target, &aspects);
        assert!(layout.used_tiles > 0);

        // Distortion = row height / (width-of-tile / aspect). Measure first placed tile.
        let rect = layout.rects[0];
        let drawn_aspect = rect.width() / rect.height();
        let true_aspect = 16.0 / 9.0;
        let distortion = (drawn_aspect / true_aspect - 1.0).abs();
        assert!(
            distortion < 0.10,
            "aspect distortion {distortion} should be < 10%"
        );
    }

    /// Too few tiles to fill the area vertically => needs_more is true.
    #[test]
    fn needs_more_when_too_few_tiles() {
        let aspects = vec![16.0 / 9.0; 2];
        let a = area(800.0, 2000.0); // very tall: 2 tiles cannot fill it.
        let layout = justified_layout(a, 200.0, &aspects);
        assert!(layout.needs_more, "should request more tiles");
        assert_eq!(layout.used_tiles, aspects.len());
    }

    /// Enough tiles to fill the area => needs_more is false.
    #[test]
    fn needs_more_false_when_enough_tiles() {
        let aspects = vec![16.0 / 9.0; 40];
        let a = area(1600.0, 900.0);
        let layout = justified_layout(a, 300.0, &aspects);
        assert!(!layout.needs_more, "should not request more tiles");
    }

    /// Surplus tiles => only a leading subset is used.
    #[test]
    fn surplus_tiles_leave_some_unused() {
        let aspects = vec![16.0 / 9.0; 60];
        let a = area(1600.0, 900.0);
        let layout = justified_layout(a, 300.0, &aspects);
        assert!(
            layout.used_tiles < aspects.len(),
            "used {} of {}",
            layout.used_tiles,
            aspects.len()
        );
    }

    /// Empty input or degenerate area yields an empty layout.
    #[test]
    fn empty_inputs_yield_empty_layout() {
        let empty = justified_layout(area(1600.0, 900.0), 300.0, &[]);
        assert_eq!(empty.used_tiles, 0);
        assert!(!empty.needs_more);

        let tiny = justified_layout(area(0.5, 0.5), 300.0, &[16.0 / 9.0; 4]);
        assert_eq!(tiny.used_tiles, 0);
        assert!(!tiny.needs_more);
    }

    /// When the height is filled, the bottom edge of the last placed row touches area.bottom().
    #[test]
    fn filled_layout_reaches_bottom() {
        let aspects = vec![16.0 / 9.0; 40];
        let a = area(1600.0, 900.0);
        let layout = justified_layout(a, 300.0, &aspects);
        let bottom = layout
            .rects
            .iter()
            .map(|r| r.bottom())
            .fold(f32::MIN, f32::max);
        assert!((bottom - a.bottom()).abs() < 0.01, "bottom {bottom}");
    }
}
