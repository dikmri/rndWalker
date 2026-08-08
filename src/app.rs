use crate::config::AppSettings;
use crate::loader::{LoadPurpose, LoadResult, PlayerLoader};
use crate::media::{choose_random_path, MediaLibrary};
use crate::updater::{self, UpdateMessage};
use eframe::egui::{Context, Key, ViewportCommand};
use egui_video::{AudioDevice, Player};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink};
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub(crate) const JAPANESE_FONT_NAME: &str = "ZenKakuGothicNew";

pub struct RndWalkerApp {
    pub(crate) settings: AppSettings,
    pub(crate) library: MediaLibrary,
    pub(crate) player: Option<Player>,
    pub(crate) video_audio_device: Option<AudioDevice>,
    pub(crate) queued_video: Option<PreparedVideo>,
    pub(crate) queued_video_folder: Option<usize>,
    pub(crate) current_video: Option<PathBuf>,
    pub(crate) queued_in_flight: bool,
    pub(crate) last_player_target_size: Option<(u32, u32)>,
    pub(crate) multiview_tiles: Vec<TileSlot>,
    pub(crate) multiview_recent: Vec<PathBuf>,
    /// Paths currently being loaded per tile slot, so concurrent picks avoid duplicates.
    pub(crate) multiview_inflight: std::collections::HashMap<usize, PathBuf>,
    pub(crate) history: Vec<PathBuf>,
    pub(crate) history_index: Option<usize>,
    pub(crate) mp3_sink: Option<MixerDeviceSink>,
    pub(crate) mp3_player: Option<rodio::Player>,
    pub(crate) current_audio: Option<PathBuf>,
    pub(crate) active_folder: Option<usize>,
    pub(crate) pending_folder: Option<Option<usize>>,
    pub(crate) show_settings: bool,
    pub(crate) folder_inputs: Vec<Vec<String>>,
    pub(crate) mp3_input: String,
    pub(crate) multiview_enabled_input: bool,
    pub(crate) multiview_video_size_input: f32,
    pub(crate) numpad_folder_switching_input: bool,
    pub(crate) selected_preset: String,
    pub(crate) active_preset: Option<String>,
    pub(crate) preset_name_input: String,
    pub(crate) volume: f32,
    pub(crate) muted: bool,
    pub(crate) fullscreen: bool,
    pub(crate) indicator: Option<TimedMessage>,
    pub(crate) info: Option<TimedMessage>,
    pub(crate) update_rx: Receiver<UpdateMessage>,
    pub(crate) update_message: UpdateMessage,
    pub(crate) loader: PlayerLoader,
    pub(crate) loader_generation: u64,
    pub(crate) next_single_request_id: u64,
    pub(crate) pending_single: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct TimedMessage {
    pub(crate) text: String,
    pub(crate) until: Instant,
}

pub(crate) struct PreparedVideo {
    pub(crate) path: PathBuf,
    pub(crate) player: Player,
    pub(crate) audio_device: Option<AudioDevice>,
    pub(crate) warning: Option<String>,
    pub(crate) preload_state: PreloadState,
}

pub(crate) struct MultiViewTile {
    pub(crate) path: PathBuf,
    pub(crate) player: Player,
    /// Replacement player loaded ahead of the video's end, swapped in instantly on finish.
    /// While `Some`, `next_requested` stays true so the preload trigger does not refire.
    pub(crate) next: Option<(PathBuf, Player)>,
    /// True while a replacement load for this tile is in flight (preload or post-finish), or while
    /// a preloaded `next` is stashed awaiting the swap. Reset to false only when the swap completes
    /// or a load finally fails, so exactly one replacement is requested per playthrough.
    pub(crate) next_requested: bool,
    /// Last decode target size applied to this tile's player (bucketed physical pixels). Used to
    /// re-target the scaler only when the on-screen rect changes bucket.
    pub(crate) last_target: Option<(u32, u32)>,
}

/// A multiview cell: either still loading its (first or replacement) player, or showing one.
// At most MULTIVIEW_MAX_TILES slots exist, so the variant size gap is irrelevant; boxing the
// player would only add indirection on the per-frame render path.
#[allow(clippy::large_enum_variant)]
pub(crate) enum TileSlot {
    /// No player yet; renders as a plain black cell. A `Tile` load is in flight for this index.
    Loading,
    /// Has a player to render. `next`/`next_requested` track its preloaded/in-flight successor.
    Ready(MultiViewTile),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PreloadState {
    Cold,
    Warming { started_at: Instant },
    SeekingToStart,
    Ready,
}

impl RndWalkerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_japanese_font(&cc.egui_ctx);

        let mut settings = AppSettings::load().unwrap_or_default();
        settings.normalize();
        if settings.restore_active_preset() {
            let _ = settings.save();
        }
        let active_preset = settings.active_preset.clone();

        let library = MediaLibrary::from_settings(&settings);
        let volume = settings.volume;
        let multiview_enabled = settings.multiview_enabled;
        let multiview_video_size = settings.multiview_video_size;
        let numpad_folder_switching = settings.numpad_folder_switching;
        let update_rx = updater::spawn_update_check();
        let loader = PlayerLoader::new(cc.egui_ctx.clone());

        let mut app = Self {
            folder_inputs: settings.mp4_folder_paths.clone(),
            mp3_input: settings.mp3_folder_path.clone(),
            settings,
            library,
            player: None,
            video_audio_device: None,
            queued_video: None,
            queued_video_folder: None,
            current_video: None,
            queued_in_flight: false,
            last_player_target_size: None,
            multiview_tiles: Vec::new(),
            multiview_recent: Vec::new(),
            multiview_inflight: std::collections::HashMap::new(),
            history: Vec::new(),
            history_index: None,
            mp3_sink: None,
            mp3_player: None,
            current_audio: None,
            active_folder: None,
            pending_folder: None,
            show_settings: false,
            selected_preset: String::new(),
            active_preset,
            preset_name_input: String::new(),
            multiview_enabled_input: multiview_enabled,
            multiview_video_size_input: multiview_video_size,
            numpad_folder_switching_input: numpad_folder_switching,
            volume,
            muted: false,
            fullscreen: false,
            indicator: None,
            info: None,
            update_rx,
            update_message: UpdateMessage::Checking,
            loader,
            loader_generation: 0,
            next_single_request_id: 0,
            pending_single: None,
        };

        app.init_audio();

        if app.settings.is_configured() {
            app.reload_library(&cc.egui_ctx);
        } else {
            app.show_settings = true;
            app.show_info("初回設定: MP4フォルダ1とMP3フォルダを選択してください");
        }

        app
    }

    pub(crate) fn init_audio(&mut self) {
        match DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                self.mp3_player = Some(rodio::Player::connect_new(sink.mixer()));
                self.mp3_sink = Some(sink);
            }
            Err(error) => self.show_info(format!("音声出力を初期化できません: {error}")),
        }
    }

    /// Invalidate any in-flight background loads. Results carrying an older generation are
    /// stopped and dropped when they arrive. Also clears the single-view in-flight markers so a
    /// fresh load can be enqueued immediately.
    pub(crate) fn bump_loader_generation(&mut self) {
        self.loader_generation = self.loader_generation.wrapping_add(1);
        self.pending_single = None;
        self.queued_in_flight = false;
    }

    pub(crate) fn reload_library(&mut self, ctx: &Context) {
        self.bump_loader_generation();
        self.settings.normalize();
        self.library = MediaLibrary::from_settings(&self.settings);
        self.active_folder = None;
        self.pending_folder = None;
        self.clear_queued_video();
        self.stop_multiview();
        self.multiview_recent.clear();
        self.history.clear();
        self.history_index = None;

        if self.library.active_video_count(None) > 0 {
            if self.settings.multiview_enabled {
                self.stop_video();
                ctx.request_repaint();
            } else {
                self.play_next_video(ctx, true);
            }
        } else {
            self.stop_video();
            self.show_info("MP4ファイルが見つかりません");
        }

        self.restart_random_audio();
    }

    pub(crate) fn reload_active_video_folder(&mut self) {
        self.bump_loader_generation();
        self.library = MediaLibrary::from_settings(&self.settings);
        self.clear_queued_video();
        self.stop_multiview();
        let count = self.library.active_video_count(self.active_folder);
        self.show_info(format!(
            "{} を再読み込みしました: {count} videos",
            folder_label(self.active_folder)
        ));
    }

    pub(crate) fn restart_random_audio(&mut self) {
        let Some(player) = self.mp3_player.as_ref() else {
            return;
        };

        player.stop();
        self.current_audio = None;
        self.play_random_audio();
    }

    pub(crate) fn play_random_audio(&mut self) {
        let Some(player) = self.mp3_player.as_ref() else {
            return;
        };
        let Some(path) = choose_random_path(&self.library.mp3_files) else {
            return;
        };

        match File::open(&path) {
            Ok(file) => match Decoder::try_from(file) {
                Ok(source) => {
                    player.stop();
                    player.set_volume(self.effective_volume());
                    player.append(source);
                    self.current_audio = Some(path);
                }
                Err(error) => self.show_info(format!("MP3をデコードできません: {error}")),
            },
            Err(error) => self.show_info(format!("MP3を開けません: {error}")),
        }
    }

    pub(crate) fn switch_folder(&mut self, folder: Option<usize>) {
        if self.library.non_empty_group_count() <= 1 {
            return;
        }
        if let Some(index) = folder {
            if self.library.mp4_sets.get(index).is_none_or(Vec::is_empty) {
                self.show_info(format!("{} は空です", folder_label(folder)));
                return;
            }
        }

        if self.settings.multiview_enabled {
            self.bump_loader_generation();
            self.active_folder = folder;
            self.pending_folder = None;
            self.stop_multiview();
            self.show_indicator(format!("再生対象: {}", folder_label(folder)));
            return;
        }

        self.pending_folder = Some(folder);
        self.clear_queued_video();
        self.show_indicator(format!("次から: {}", folder_label(folder)));
    }

    pub(crate) fn activate_preset(&mut self, name: &str, ctx: &Context) {
        let Some(preset) = self.settings.presets.get(name).cloned() else {
            return;
        };

        self.settings.mp4_folder_paths = preset.mp4_folder_paths;
        self.settings.mp3_folder_path = preset.mp3_folder_path;
        self.settings.normalize();
        self.folder_inputs = self.settings.mp4_folder_paths.clone();
        self.mp3_input = self.settings.mp3_folder_path.clone();
        self.active_preset = Some(name.to_owned());
        self.settings.active_preset = self.active_preset.clone();

        if let Err(error) = self.settings.save() {
            self.show_info(format!("プリセット切替の保存失敗: {error}"));
        }
        self.reload_library(ctx);
        self.show_indicator(format!("プリセット: {name}"));
    }

    pub(crate) fn apply_pending_folder_switch(&mut self) {
        if let Some(folder) = self.pending_folder.take() {
            self.active_folder = folder;
            self.show_info(format!("再生対象: {}", folder_label(folder)));
        }
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.settings.volume = self.volume;
        let _ = self.settings.save();
        self.apply_volume();
        self.show_indicator(format!("音量 {}%", (self.volume * 100.0).round() as u32));
    }

    pub(crate) fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.apply_volume();
        if self.muted {
            self.show_indicator("ミュート");
        } else {
            self.show_indicator(format!("音量 {}%", (self.volume * 100.0).round() as u32));
        }
    }

    pub(crate) fn apply_volume(&mut self) {
        let volume = self.effective_volume();
        if let Some(player) = self.player.as_mut() {
            player.options.set_audio_volume(volume);
        }
        if let Some(prepared) = self.queued_video.as_mut() {
            prepared.player.options.set_audio_volume(0.0);
        }
        for slot in &mut self.multiview_tiles {
            if let TileSlot::Ready(tile) = slot {
                tile.player.options.set_audio_volume(0.0);
            }
        }
        if let Some(player) = self.mp3_player.as_ref() {
            player.set_volume(volume);
        }
    }

    pub(crate) fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    pub(crate) fn show_indicator(&mut self, text: impl Into<String>) {
        self.indicator = Some(TimedMessage {
            text: text.into(),
            until: Instant::now() + Duration::from_millis(1500),
        });
    }

    pub(crate) fn show_info(&mut self, text: impl Into<String>) {
        self.info = Some(TimedMessage {
            text: text.into(),
            until: Instant::now() + Duration::from_millis(2600),
        });
    }

    pub(crate) fn clear_expired_messages(&mut self) {
        let now = Instant::now();
        if self
            .indicator
            .as_ref()
            .is_some_and(|message| message.until <= now)
        {
            self.indicator = None;
        }
        if self
            .info
            .as_ref()
            .is_some_and(|message| message.until <= now)
        {
            self.info = None;
        }
    }

    pub(crate) fn poll_update_status(&mut self) {
        while let Ok(message) = self.update_rx.try_recv() {
            match &message {
                UpdateMessage::Updated(version) => {
                    self.show_info(format!("更新完了: v{version}. 再起動してください"));
                }
                UpdateMessage::UpToDate(version) => {
                    self.show_info(format!("最新バージョンです: v{version}"));
                }
                UpdateMessage::Skipped(reason) => self.show_info(reason.clone()),
                UpdateMessage::Failed(error) => {
                    self.show_info(format!("更新チェック失敗: {error}"));
                }
                UpdateMessage::Checking => {}
            }
            self.update_message = message;
        }
    }

    /// Drain finished background loads. Stale-generation results are stopped and dropped; the rest
    /// are dispatched to the single-view / multiview handlers.
    pub(crate) fn drain_loader_results(&mut self, ctx: &Context) {
        while let Some(result) = self.loader.try_recv() {
            if result.request.generation != self.loader_generation {
                if let Ok(player) = result.player {
                    self.loader.dispose(player);
                }
                continue;
            }

            let LoadResult { request, player } = result;
            let path = request.path;
            match request.purpose {
                LoadPurpose::Single {
                    add_to_history,
                    request_id,
                } => self.on_single_loaded(request_id, path, add_to_history, player),
                LoadPurpose::Queued { folder } => self.on_queued_loaded(path, folder, player),
                LoadPurpose::Tile { index } => {
                    self.on_tile_loaded(ctx, index, path, request.attempts, player)
                }
            }
        }
    }

    pub(crate) fn handle_keyboard(&mut self, ctx: &Context) {
        if self.show_settings {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.show_settings = false;
            }
            return;
        }

        if let Some(name) = self.pressed_preset_name(ctx) {
            self.activate_preset(&name, ctx);
            return;
        }

        if ctx.input(|input| input.key_pressed(Key::F5)) {
            self.reload_active_video_folder();
        }
        if ctx.input(|input| input.key_pressed(Key::F11)) {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
        }
        if ctx.input(|input| input.key_pressed(Key::Escape)) && self.fullscreen {
            self.fullscreen = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
        }
        if ctx.input(|input| input.key_pressed(Key::M)) {
            self.toggle_mute();
        }
        if ctx.input(|input| input.key_pressed(Key::W)) {
            self.set_volume(self.volume + 0.05);
        }
        if ctx.input(|input| input.key_pressed(Key::S)) {
            self.set_volume(self.volume - 0.05);
        }
        if ctx.input(|input| input.key_pressed(Key::D)) {
            if self.settings.multiview_enabled {
                self.shuffle_multiview(ctx);
            } else {
                self.play_next_video(ctx, true);
            }
        }
        if ctx.input(|input| input.key_pressed(Key::A)) {
            if self.settings.multiview_enabled {
                self.show_info("マルチビューでは前の動画へ戻れません");
            } else {
                self.previous_video(ctx);
            }
        }
        if ctx.input(|input| input.key_pressed(Key::ArrowLeft)) {
            self.switch_folder(Some(0));
        }
        if ctx.input(|input| input.key_pressed(Key::ArrowUp)) {
            self.switch_folder(Some(1));
        }
        if ctx.input(|input| input.key_pressed(Key::ArrowRight)) {
            self.switch_folder(Some(2));
        }
        if ctx.input(|input| input.key_pressed(Key::ArrowDown)) {
            self.switch_folder(None);
        }
        if self.settings.numpad_folder_switching {
            if ctx.input(|input| input.key_pressed(Key::Num1)) {
                self.switch_folder(Some(0));
            }
            if ctx.input(|input| input.key_pressed(Key::Num2)) {
                self.switch_folder(Some(1));
            }
            if ctx.input(|input| input.key_pressed(Key::Num3)) {
                self.switch_folder(Some(2));
            }
            if ctx.input(|input| input.key_pressed(Key::Num4)) {
                self.switch_folder(Some(3));
            }
            if ctx.input(|input| input.key_pressed(Key::Num5)) {
                self.switch_folder(Some(4));
            }
            if ctx.input(|input| input.key_pressed(Key::Num6)) {
                self.switch_folder(Some(5));
            }
            if ctx.input(|input| input.key_pressed(Key::Num7)) {
                self.switch_folder(Some(6));
            }
            if ctx.input(|input| input.key_pressed(Key::Num8)) {
                self.switch_folder(Some(7));
            }
            if ctx.input(|input| input.key_pressed(Key::Num9)) {
                self.switch_folder(Some(8));
            }
            if ctx.input(|input| input.key_pressed(Key::Num0)) {
                self.switch_folder(None);
            }
        }

        if self.settings.multiview_enabled {
            let scroll = ctx.input(|input| input.raw_scroll_delta.y);
            if scroll != 0.0 {
                self.adjust_multiview_video_size(scroll);
            }
        }
    }

    fn pressed_preset_name(&self, ctx: &Context) -> Option<String> {
        self.settings.presets.iter().find_map(|(name, preset)| {
            let hotkey = preset.hotkey?;
            let key = function_key(hotkey)?;
            ctx.input(|input| input.key_pressed(key))
                .then(|| name.clone())
        })
    }

    /// Mouse-wheel handler for multiview: scroll up enlarges tiles, scroll down shrinks them.
    /// The layout reflows immediately; tiles themselves are not reloaded.
    fn adjust_multiview_video_size(&mut self, scroll_delta: f32) {
        use crate::config::{MAX_MULTIVIEW_VIDEO_SIZE, MIN_MULTIVIEW_VIDEO_SIZE};

        let new_size = (self.settings.multiview_video_size + scroll_delta * 0.4)
            .round()
            .clamp(MIN_MULTIVIEW_VIDEO_SIZE, MAX_MULTIVIEW_VIDEO_SIZE);
        if new_size == self.settings.multiview_video_size {
            return;
        }

        self.settings.multiview_video_size = new_size;
        let _ = self.settings.save();
        self.show_indicator(format!("動画サイズ {}px", new_size as u32));
    }
}

impl eframe::App for RndWalkerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));
        self.drain_loader_results(ctx);
        self.clear_expired_messages();
        self.poll_update_status();
        self.handle_keyboard(ctx);

        if self
            .mp3_player
            .as_ref()
            .is_some_and(|player| player.empty())
        {
            self.play_random_audio();
        }

        if self.settings.multiview_enabled {
            self.draw_multiview(ctx);
        } else {
            self.draw_player(ctx);
        }
        self.draw_overlays(ctx);
        self.draw_settings(ctx);
    }
}

/// Round a logical size (in physical pixels) UP to a multiple of 64, so the decoder's scaler only
/// rebuilds when the bucketed value changes instead of on every pixel of an interactive resize.
/// A zero (or negative) dimension buckets to 64 to keep a valid, non-zero decode box.
pub(crate) fn bucket_target_size(width: f32, height: f32) -> (u32, u32) {
    (bucket_dimension(width), bucket_dimension(height))
}

fn bucket_dimension(value: f32) -> u32 {
    const BUCKET: u32 = 64;
    let pixels = value.ceil().max(1.0) as u32;
    pixels.div_ceil(BUCKET) * BUCKET
}

pub(crate) fn folder_label(folder: Option<usize>) -> String {
    match folder {
        Some(index) => format!("フォルダ{}", index + 1),
        None => "全フォルダ".to_owned(),
    }
}

fn function_key(number: u8) -> Option<Key> {
    Some(match number {
        1 => Key::F1,
        2 => Key::F2,
        3 => Key::F3,
        4 => Key::F4,
        5 => Key::F5,
        6 => Key::F6,
        7 => Key::F7,
        8 => Key::F8,
        9 => Key::F9,
        10 => Key::F10,
        11 => Key::F11,
        12 => Key::F12,
        _ => return None,
    })
}

pub(crate) fn install_japanese_font(ctx: &Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        JAPANESE_FONT_NAME.to_owned(),
        FontData::from_static(include_bytes!(
            "../assets/fonts/ZenKakuGothicNew-Regular.ttf"
        )),
    );

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, JAPANESE_FONT_NAME.to_owned());
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::bucket_target_size;

    #[test]
    fn bucket_rounds_up_to_multiple_of_64() {
        assert_eq!(bucket_target_size(1.0, 64.0), (64, 64));
        assert_eq!(bucket_target_size(65.0, 128.0), (128, 128));
        assert_eq!(bucket_target_size(1920.0, 1080.0), (1920, 1088));
    }

    #[test]
    fn bucket_clamps_zero_and_negative_to_one_bucket() {
        assert_eq!(bucket_target_size(0.0, -10.0), (64, 64));
    }

    #[test]
    fn bucket_is_stable_within_a_bucket() {
        // Values inside the same 64px band collapse to one output, avoiding scaler churn.
        assert_eq!(
            bucket_target_size(129.0, 200.0),
            bucket_target_size(192.0, 256.0)
        );
    }
}
