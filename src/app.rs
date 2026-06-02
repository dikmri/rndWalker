use crate::config::{AppSettings, FolderPreset, MAX_SUB_FOLDERS, NUM_GROUPS};
use crate::media::{choose_random_path, MediaLibrary};
use crate::updater::{self, UpdateMessage};
use eframe::egui::{
    self, Align, Align2, Area, CentralPanel, Color32, ComboBox, Context, Frame, Id, Key, Layout,
    Margin, Rect, RichText, ScrollArea, TextEdit, Vec2, ViewportCommand, Window,
};
use egui_video::{AudioDevice, Player, PlayerState};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink};
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub struct RndWalkerApp {
    settings: AppSettings,
    library: MediaLibrary,
    player: Option<Player>,
    video_audio_device: Option<AudioDevice>,
    current_video: Option<PathBuf>,
    history: Vec<PathBuf>,
    history_index: Option<usize>,
    mp3_sink: Option<MixerDeviceSink>,
    mp3_player: Option<rodio::Player>,
    current_audio: Option<PathBuf>,
    active_folder: Option<usize>,
    pending_folder: Option<Option<usize>>,
    show_settings: bool,
    folder_inputs: Vec<Vec<String>>,
    visible_sub_folders: Vec<Vec<bool>>,
    mp3_input: String,
    selected_preset: String,
    preset_name_input: String,
    volume: f32,
    muted: bool,
    fullscreen: bool,
    indicator: Option<TimedMessage>,
    info: Option<TimedMessage>,
    update_rx: Receiver<UpdateMessage>,
    update_message: UpdateMessage,
}

#[derive(Debug, Clone)]
struct TimedMessage {
    text: String,
    until: Instant,
}

impl RndWalkerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut settings = AppSettings::load().unwrap_or_default();
        settings.normalize();

        let library = MediaLibrary::from_settings(&settings);
        let volume = settings.volume;
        let update_rx = updater::spawn_update_check();

        let mut app = Self {
            folder_inputs: settings.mp4_folder_paths.clone(),
            visible_sub_folders: visible_from_groups(&settings.mp4_folder_paths),
            mp3_input: settings.mp3_folder_path.clone(),
            settings,
            library,
            player: None,
            video_audio_device: None,
            current_video: None,
            history: Vec::new(),
            history_index: None,
            mp3_sink: None,
            mp3_player: None,
            current_audio: None,
            active_folder: None,
            pending_folder: None,
            show_settings: false,
            selected_preset: String::new(),
            preset_name_input: String::new(),
            volume,
            muted: false,
            fullscreen: false,
            indicator: None,
            info: None,
            update_rx,
            update_message: UpdateMessage::Checking,
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

    fn init_audio(&mut self) {
        match DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                self.mp3_player = Some(rodio::Player::connect_new(sink.mixer()));
                self.mp3_sink = Some(sink);
            }
            Err(error) => self.show_info(format!("音声出力を初期化できません: {error}")),
        }
    }

    fn reload_library(&mut self, ctx: &Context) {
        self.settings.normalize();
        self.library = MediaLibrary::from_settings(&self.settings);
        self.active_folder = None;
        self.pending_folder = None;
        self.history.clear();
        self.history_index = None;

        if self.library.active_video_count(None) > 0 {
            self.play_next_video(ctx, true);
        } else {
            self.stop_video();
            self.show_info("MP4ファイルが見つかりません");
        }

        self.restart_random_audio();
    }

    fn reload_active_video_folder(&mut self) {
        self.library = MediaLibrary::from_settings(&self.settings);
        let count = self.library.active_video_count(self.active_folder);
        self.show_info(format!(
            "{} を再読み込みしました: {count} videos",
            folder_label(self.active_folder)
        ));
    }

    fn play_next_video(&mut self, ctx: &Context, add_to_history: bool) {
        let videos = self.library.active_videos(self.active_folder);
        let Some(path) = choose_random_path(&videos) else {
            self.show_info("再生できるMP4ファイルがありません");
            return;
        };

        self.load_video(ctx, path, add_to_history);
    }

    fn load_video(&mut self, ctx: &Context, path: PathBuf, add_to_history: bool) {
        self.stop_video();

        let input_path = path.to_string_lossy().to_string();
        match Player::new(ctx, &input_path) {
            Ok(mut player) => {
                player.options.looping = false;
                player.options.set_audio_volume(self.effective_volume());

                match AudioDevice::new() {
                    Ok(mut audio_device) => {
                        if let Err(error) = player.add_audio(&mut audio_device) {
                            self.show_info(format!("動画音声を初期化できません: {error}"));
                        }
                        self.video_audio_device = Some(audio_device);
                    }
                    Err(error) => {
                        self.show_info(format!("動画音声デバイスを初期化できません: {error}"))
                    }
                }

                player.start();
                self.current_video = Some(path.clone());
                self.player = Some(player);

                if add_to_history {
                    if let Some(index) = self.history_index {
                        self.history.truncate(index + 1);
                    }
                    self.history.push(path);
                    self.history_index = self.history.len().checked_sub(1);
                }
            }
            Err(error) => {
                self.show_info(format!("動画を読み込めません: {error}"));
            }
        }
    }

    fn stop_video(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.stop();
        }
        self.player = None;
        self.video_audio_device = None;
        self.current_video = None;
    }

    fn previous_video(&mut self, ctx: &Context) {
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

    fn restart_random_audio(&mut self) {
        let Some(player) = self.mp3_player.as_ref() else {
            return;
        };

        player.stop();
        self.current_audio = None;
        self.play_random_audio();
    }

    fn play_random_audio(&mut self) {
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

    fn switch_folder(&mut self, folder: Option<usize>) {
        if self.library.non_empty_group_count() <= 1 {
            return;
        }
        if let Some(index) = folder {
            if self.library.mp4_sets.get(index).is_none_or(Vec::is_empty) {
                self.show_info(format!("{} は空です", folder_label(folder)));
                return;
            }
        }

        self.pending_folder = Some(folder);
        self.show_indicator(format!("Next: {}", folder_label(folder)));
    }

    fn apply_pending_folder_switch(&mut self) {
        if let Some(folder) = self.pending_folder.take() {
            self.active_folder = folder;
            self.show_info(format!("Now playing: {}", folder_label(folder)));
        }
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        self.settings.volume = self.volume;
        let _ = self.settings.save();
        self.apply_volume();
        self.show_indicator(format!("Volume {}%", (self.volume * 100.0).round() as u32));
    }

    fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        self.apply_volume();
        if self.muted {
            self.show_indicator("Muted");
        } else {
            self.show_indicator(format!("Volume {}%", (self.volume * 100.0).round() as u32));
        }
    }

    fn apply_volume(&mut self) {
        let volume = self.effective_volume();
        if let Some(player) = self.player.as_mut() {
            player.options.set_audio_volume(volume);
        }
        if let Some(player) = self.mp3_player.as_ref() {
            player.set_volume(volume);
        }
    }

    fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    fn show_indicator(&mut self, text: impl Into<String>) {
        self.indicator = Some(TimedMessage {
            text: text.into(),
            until: Instant::now() + Duration::from_millis(1500),
        });
    }

    fn show_info(&mut self, text: impl Into<String>) {
        self.info = Some(TimedMessage {
            text: text.into(),
            until: Instant::now() + Duration::from_millis(2600),
        });
    }

    fn clear_expired_messages(&mut self) {
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

    fn poll_update_status(&mut self) {
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

    fn handle_keyboard(&mut self, ctx: &Context) {
        if self.show_settings {
            if ctx.input(|input| input.key_pressed(Key::Escape)) {
                self.show_settings = false;
            }
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
            self.play_next_video(ctx, true);
        }
        if ctx.input(|input| input.key_pressed(Key::A)) {
            self.previous_video(ctx);
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
    }

    fn draw_player(&mut self, ctx: &Context) {
        CentralPanel::default()
            .frame(Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let mut finished = false;
                if let Some(player) = self.player.as_mut() {
                    let target = fit_rect(rect, player.size);
                    player.render_frame_at(ui, target);
                    player.process_state();

                    finished = matches!(player.player_state.get(), PlayerState::EndOfFile)
                        || (matches!(player.player_state.get(), PlayerState::Stopped)
                            && player.duration_ms > 0
                            && player.elapsed_ms() + 250 >= player.duration_ms);
                } else {
                    ui.with_layout(
                        Layout::centered_and_justified(egui::Direction::TopDown),
                        |ui| {
                            ui.label(
                                RichText::new("No video loaded")
                                    .color(Color32::from_gray(180))
                                    .size(22.0),
                            );
                        },
                    );
                }

                if finished {
                    self.apply_pending_folder_switch();
                    self.play_next_video(ctx, true);
                }
            });
    }

    fn draw_overlays(&mut self, ctx: &Context) {
        if let Some(message) = &self.indicator {
            Area::new(Id::new("indicator"))
                .anchor(Align2::CENTER_TOP, [0.0, 20.0])
                .show(ctx, |ui| {
                    overlay_frame().show(ui, |ui| {
                        ui.label(
                            RichText::new(&message.text)
                                .color(Color32::WHITE)
                                .size(18.0),
                        );
                    });
                });
        }

        if let Some(message) = &self.info {
            Area::new(Id::new("info"))
                .anchor(Align2::CENTER_BOTTOM, [0.0, -20.0])
                .show(ctx, |ui| {
                    overlay_frame().show(ui, |ui| {
                        ui.label(
                            RichText::new(&message.text)
                                .color(Color32::WHITE)
                                .size(14.0),
                        );
                    });
                });
        }

        Area::new(Id::new("settings_button"))
            .anchor(Align2::RIGHT_TOP, [-12.0, 12.0])
            .show(ctx, |ui| {
                if ui.button("Settings").clicked() {
                    self.open_settings();
                }
            });

        Area::new(Id::new("status"))
            .anchor(Align2::LEFT_TOP, [12.0, 12.0])
            .show(ctx, |ui| {
                let count = self.library.active_video_count(self.active_folder);
                let update = match &self.update_message {
                    UpdateMessage::Checking => "checking updates",
                    UpdateMessage::Skipped(_) => "dev build",
                    UpdateMessage::UpToDate(_) => "up to date",
                    UpdateMessage::Updated(_) => "updated",
                    UpdateMessage::Failed(_) => "update check failed",
                };
                overlay_frame().show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} | {count} videos | {update}",
                            folder_label(self.active_folder)
                        ))
                        .color(Color32::WHITE)
                        .size(13.0),
                    );
                });
            });
    }

    fn open_settings(&mut self) {
        self.folder_inputs = self.settings.mp4_folder_paths.clone();
        self.visible_sub_folders = visible_from_groups(&self.folder_inputs);
        self.mp3_input = self.settings.mp3_folder_path.clone();
        self.selected_preset.clear();
        self.preset_name_input.clear();
        self.show_settings = true;
    }

    fn draw_settings(&mut self, ctx: &Context) {
        if !self.show_settings {
            return;
        }

        let mut open = self.show_settings;
        let mut save_clicked = false;
        Window::new("rndWalker Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(720.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                    for group in 0..NUM_GROUPS {
                        ui.heading(format!("MP4 Folder {}", group + 1));
                        if group > 0 && ui.button("Clear group").clicked() {
                            for sub in 0..MAX_SUB_FOLDERS {
                                self.folder_inputs[group][sub].clear();
                                self.visible_sub_folders[group][sub] = sub == 0;
                            }
                        }

                        for sub in 0..MAX_SUB_FOLDERS {
                            if sub > 0 && !self.visible_sub_folders[group][sub] {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                let label = if sub == 0 {
                                    "Base".to_owned()
                                } else {
                                    format!("Sub {}", sub + 1)
                                };
                                ui.label(label);
                                ui.add_sized(
                                    [420.0, 22.0],
                                    TextEdit::singleline(&mut self.folder_inputs[group][sub])
                                        .interactive(false),
                                );
                                if ui.button("Browse").clicked() {
                                    if let Some(path) = pick_folder() {
                                        self.folder_inputs[group][sub] = path;
                                    }
                                }
                                if sub > 0 && ui.button("-").clicked() {
                                    self.folder_inputs[group][sub].clear();
                                    self.visible_sub_folders[group][sub] = false;
                                }
                            });
                        }

                        if ui.button("+ Add sub folder").clicked() {
                            if let Some(slot) = (1..MAX_SUB_FOLDERS)
                                .find(|slot| !self.visible_sub_folders[group][*slot])
                            {
                                self.visible_sub_folders[group][slot] = true;
                            }
                        }
                        ui.separator();
                    }

                    ui.heading("MP3 Folder");
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [520.0, 22.0],
                            TextEdit::singleline(&mut self.mp3_input).interactive(false),
                        );
                        if ui.button("Browse").clicked() {
                            if let Some(path) = pick_folder() {
                                self.mp3_input = path;
                            }
                        }
                    });

                    ui.separator();
                    self.draw_presets(ui);
                });

                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        self.show_settings = false;
                    }
                    if ui.button("Save").clicked() {
                        save_clicked = true;
                    }
                });
            });

        if save_clicked {
            self.save_settings_from_inputs(ctx);
            open = self.show_settings;
        }
        self.show_settings = open && self.show_settings;
    }

    fn draw_presets(&mut self, ui: &mut egui::Ui) {
        ui.heading("Presets");
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("preset_select")
                .selected_text(if self.selected_preset.is_empty() {
                    "-- Select Preset --"
                } else {
                    &self.selected_preset
                })
                .show_ui(ui, |ui| {
                    let names: Vec<String> = self.settings.presets.keys().cloned().collect();
                    for name in names {
                        ui.selectable_value(&mut self.selected_preset, name.clone(), &name);
                    }
                });

            if ui.button("Load").clicked() {
                self.load_selected_preset();
            }
            if ui.button("Delete").clicked() {
                self.delete_selected_preset();
            }
        });

        ui.horizontal(|ui| {
            ui.add_sized(
                [320.0, 22.0],
                TextEdit::singleline(&mut self.preset_name_input).hint_text("Preset name"),
            );
            if ui.button("Save as Preset").clicked() {
                self.save_current_as_preset();
            }
        });
    }

    fn save_current_as_preset(&mut self) {
        let name = self.preset_name_input.trim().to_owned();
        if name.is_empty() {
            self.show_info("プリセット名を入力してください");
            return;
        }

        self.settings.presets.insert(
            name.clone(),
            FolderPreset {
                mp4_folder_paths: self.folder_inputs.clone(),
                mp3_folder_path: self.mp3_input.clone(),
            },
        );
        if let Err(error) = self.settings.save() {
            self.show_info(format!("プリセット保存失敗: {error}"));
            return;
        }

        self.selected_preset = name;
        self.preset_name_input.clear();
        self.show_info(format!("プリセット保存: {}", self.selected_preset));
    }

    fn load_selected_preset(&mut self) {
        if self.selected_preset.is_empty() {
            self.show_info("プリセットを選択してください");
            return;
        }

        let Some(preset) = self.settings.presets.get(&self.selected_preset).cloned() else {
            self.show_info("プリセットが見つかりません");
            return;
        };

        self.folder_inputs = preset.mp4_folder_paths;
        self.visible_sub_folders = visible_from_groups(&self.folder_inputs);
        self.mp3_input = preset.mp3_folder_path;
        self.show_info(format!("プリセット読込: {}", self.selected_preset));
    }

    fn delete_selected_preset(&mut self) {
        if self.selected_preset.is_empty() {
            self.show_info("プリセットを選択してください");
            return;
        }

        let name = self.selected_preset.clone();
        self.settings.presets.remove(&name);
        if let Err(error) = self.settings.save() {
            self.show_info(format!("プリセット削除失敗: {error}"));
            return;
        }
        self.selected_preset.clear();
        self.show_info(format!("プリセット削除: {name}"));
    }

    fn save_settings_from_inputs(&mut self, ctx: &Context) {
        if self.folder_inputs[0][0].trim().is_empty() || self.mp3_input.trim().is_empty() {
            self.show_info("MP4 Folder 1 と MP3 Folder は必須です");
            return;
        }

        self.settings.mp4_folder_paths = self.folder_inputs.clone();
        self.settings.mp3_folder_path = self.mp3_input.clone();
        self.settings.volume = self.volume;
        self.settings.normalize();

        if let Err(error) = self.settings.save() {
            self.show_info(format!("設定保存失敗: {error}"));
            return;
        }

        self.show_settings = false;
        self.reload_library(ctx);
        self.show_info("設定を保存しました");
    }
}

impl eframe::App for RndWalkerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));
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

        self.draw_player(ctx);
        self.draw_overlays(ctx);
        self.draw_settings(ctx);
    }
}

fn pick_folder() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string())
}

fn folder_label(folder: Option<usize>) -> String {
    match folder {
        Some(index) => format!("Folder {}", index + 1),
        None => "All Folders".to_owned(),
    }
}

fn visible_from_groups(groups: &[Vec<String>]) -> Vec<Vec<bool>> {
    let mut visible = vec![vec![false; MAX_SUB_FOLDERS]; NUM_GROUPS];
    for group in 0..NUM_GROUPS {
        visible[group][0] = true;
        for sub in 1..MAX_SUB_FOLDERS {
            visible[group][sub] = groups
                .get(group)
                .and_then(|folders| folders.get(sub))
                .is_some_and(|path| !path.trim().is_empty());
        }
    }
    visible
}

fn fit_rect(container: Rect, content_size: Vec2) -> Rect {
    if content_size.x <= 0.0 || content_size.y <= 0.0 {
        return container;
    }

    let scale = (container.width() / content_size.x).min(container.height() / content_size.y);
    Rect::from_center_size(container.center(), content_size * scale)
}

fn overlay_frame() -> Frame {
    Frame::none()
        .fill(Color32::from_black_alpha(180))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
}
