use crate::app::RndWalkerApp;
use crate::config::{FolderPreset, DEFAULT_GROUPS, NUM_GROUPS, PRESET_FUNCTION_KEYS};
use eframe::egui::{
    self, Align, Align2, Area, Color32, ComboBox, Context, FontId, Frame, Id, Layout, Margin,
    RichText, ScrollArea, Sense, Stroke, TextEdit, Vec2,
};

pub(crate) fn pick_folder() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string())
}

pub(crate) fn settings_button(ui: &mut egui::Ui) -> egui::Response {
    let size = Vec2::new(72.0, 34.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let fill_alpha = if hovered { 230 } else { 26 };
    let stroke_alpha = if hovered { 220 } else { 45 };
    let text_alpha = if hovered { 255 } else { 105 };

    ui.painter().rect(
        rect,
        egui::Rounding::same(17.0),
        Color32::from_black_alpha(fill_alpha),
        Stroke::new(1.0, Color32::from_white_alpha(stroke_alpha)),
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "設定",
        FontId::proportional(15.0),
        Color32::from_white_alpha(text_alpha),
    );

    response.on_hover_text("設定を開く")
}

pub(crate) fn overlay_frame() -> Frame {
    Frame::none()
        .fill(Color32::from_black_alpha(180))
        .rounding(egui::Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
}

impl RndWalkerApp {
    pub(crate) fn draw_overlays(&mut self, ctx: &Context) {
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
            .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
            .show(ctx, |ui| {
                if settings_button(ui).clicked() {
                    self.open_settings();
                }
            });
    }

    pub(crate) fn open_settings(&mut self) {
        self.folder_inputs = self.settings.mp4_folder_paths.clone();
        self.mp3_input = self.settings.mp3_folder_path.clone();
        self.multiview_enabled_input = self.settings.multiview_enabled;
        self.multiview_video_size_input = self.settings.multiview_video_size;
        self.numpad_folder_switching_input = self.settings.numpad_folder_switching;
        self.selected_preset = self.active_preset.clone().unwrap_or_default();
        self.preset_name_input.clear();
        self.show_settings = true;
    }

    pub(crate) fn draw_settings(&mut self, ctx: &Context) {
        use crate::config::{MAX_MULTIVIEW_VIDEO_SIZE, MIN_MULTIVIEW_VIDEO_SIZE};

        if !self.show_settings {
            return;
        }

        let mut open = self.show_settings;
        let mut save_clicked = false;
        egui::Window::new("rndWalker 設定")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(720.0)
            .show(ctx, |ui| {
                ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                    let group_count = if self.numpad_folder_switching_input {
                        NUM_GROUPS
                    } else {
                        DEFAULT_GROUPS
                    };
                    for group in 0..group_count {
                        ui.heading(format!("MP4フォルダ {}", group + 1));
                        if group > 0 && ui.button("このグループをクリア").clicked() {
                            self.folder_inputs[group] = vec![String::new()];
                        }

                        let mut remove_sub: Option<usize> = None;
                        let sub_count = self.folder_inputs[group].len();
                        for sub in 0..sub_count {
                            ui.horizontal(|ui| {
                                let label = if sub == 0 {
                                    "基本".to_owned()
                                } else {
                                    format!("追加 {}", sub + 1)
                                };
                                ui.label(label);
                                ui.add_sized(
                                    [420.0, 22.0],
                                    TextEdit::singleline(&mut self.folder_inputs[group][sub])
                                        .interactive(false),
                                );
                                if ui.button("参照").clicked() {
                                    if let Some(path) = pick_folder() {
                                        self.folder_inputs[group][sub] = path;
                                    }
                                }
                                if sub > 0 && ui.button("-").clicked() {
                                    remove_sub = Some(sub);
                                }
                            });
                        }
                        if let Some(sub) = remove_sub {
                            self.folder_inputs[group].remove(sub);
                        }

                        if ui.button("+ 追加フォルダ").clicked() {
                            self.folder_inputs[group].push(String::new());
                        }
                        ui.separator();
                    }

                    ui.heading("MP3フォルダ");
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [520.0, 22.0],
                            TextEdit::singleline(&mut self.mp3_input).interactive(false),
                        );
                        if ui.button("参照").clicked() {
                            if let Some(path) = pick_folder() {
                                self.mp3_input = path;
                            }
                        }
                    });

                    ui.separator();
                    ui.heading("マルチビュー");
                    ui.checkbox(
                        &mut self.multiview_enabled_input,
                        "マルチビューを有効にする",
                    );
                    ui.add(
                        egui::Slider::new(
                            &mut self.multiview_video_size_input,
                            MIN_MULTIVIEW_VIDEO_SIZE..=MAX_MULTIVIEW_VIDEO_SIZE,
                        )
                        .text("動画サイズ")
                        .suffix(" px"),
                    );

                    ui.separator();
                    ui.heading("キーボード");
                    ui.checkbox(
                        &mut self.numpad_folder_switching_input,
                        "テンキーでフォルダ切替と9グループを有効にする (1〜9 = 各フォルダ、0 = 全フォルダ)",
                    );

                    ui.separator();
                    self.draw_presets(ui);
                });

                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("キャンセル").clicked() {
                        self.show_settings = false;
                    }
                    if ui.button("保存").clicked() {
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

    pub(crate) fn draw_presets(&mut self, ui: &mut egui::Ui) {
        ui.heading("プリセット");
        ui.horizontal(|ui| {
            ComboBox::from_id_salt("preset_select")
                .selected_text(if self.selected_preset.is_empty() {
                    "-- プリセットを選択 --"
                } else {
                    &self.selected_preset
                })
                .show_ui(ui, |ui| {
                    let names: Vec<String> = self.settings.presets.keys().cloned().collect();
                    for name in names {
                        ui.selectable_value(&mut self.selected_preset, name.clone(), &name);
                    }
                });

            if ui.button("読込").clicked() {
                self.load_selected_preset();
            }
            if ui.button("削除").clicked() {
                self.delete_selected_preset();
            }
        });

        ui.label("ショートカット");
        let names: Vec<String> = self.settings.presets.keys().cloned().collect();
        for name in names {
            let current = self
                .settings
                .presets
                .get(&name)
                .and_then(|preset| preset.hotkey);
            let mut hotkey = current;
            ui.horizontal(|ui| {
                ui.label(&name);
                ComboBox::from_id_salt(("preset_hotkey", &name))
                    .selected_text(preset_hotkey_label(hotkey))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut hotkey, None, "なし");
                        for key in PRESET_FUNCTION_KEYS {
                            ui.selectable_value(&mut hotkey, Some(key), format!("F{key}"));
                        }
                    });
            });
            if hotkey != current {
                self.assign_preset_hotkey(&name, hotkey);
            }
        }

        ui.horizontal(|ui| {
            ui.add_sized(
                [320.0, 22.0],
                TextEdit::singleline(&mut self.preset_name_input).hint_text("プリセット名"),
            );
            if ui.button("プリセットとして保存").clicked() {
                self.save_current_as_preset();
            }
        });
    }

    pub(crate) fn save_current_as_preset(&mut self) {
        let name = self.preset_name_input.trim().to_owned();
        if name.is_empty() {
            self.show_info("プリセット名を入力してください");
            return;
        }

        let hotkey = self
            .settings
            .presets
            .get(&name)
            .and_then(|preset| preset.hotkey);
        self.settings.presets.insert(
            name.clone(),
            FolderPreset {
                mp4_folder_paths: self.folder_inputs.clone(),
                mp3_folder_path: self.mp3_input.clone(),
                hotkey,
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

    pub(crate) fn load_selected_preset(&mut self) {
        if self.selected_preset.is_empty() {
            self.show_info("プリセットを選択してください");
            return;
        }

        let Some(preset) = self.settings.presets.get(&self.selected_preset).cloned() else {
            self.show_info("プリセットが見つかりません");
            return;
        };

        self.folder_inputs = preset.mp4_folder_paths;
        self.mp3_input = preset.mp3_folder_path;
        self.show_info(format!("プリセット読込: {}", self.selected_preset));
    }

    pub(crate) fn delete_selected_preset(&mut self) {
        if self.selected_preset.is_empty() {
            self.show_info("プリセットを選択してください");
            return;
        }

        let name = self.selected_preset.clone();
        self.settings.presets.remove(&name);
        if self.active_preset.as_deref() == Some(name.as_str()) {
            self.active_preset = None;
            self.settings.active_preset = None;
        }
        if let Err(error) = self.settings.save() {
            self.show_info(format!("プリセット削除失敗: {error}"));
            return;
        }
        self.selected_preset.clear();
        self.show_info(format!("プリセット削除: {name}"));
    }

    fn assign_preset_hotkey(&mut self, name: &str, hotkey: Option<u8>) {
        if let Some(hotkey) = hotkey {
            for (preset_name, preset) in &mut self.settings.presets {
                if preset_name != name && preset.hotkey == Some(hotkey) {
                    preset.hotkey = None;
                }
            }
        }
        if let Some(preset) = self.settings.presets.get_mut(name) {
            preset.hotkey = hotkey;
        }
        if let Err(error) = self.settings.save() {
            self.show_info(format!("ショートカット保存失敗: {error}"));
        }
    }

    pub(crate) fn save_settings_from_inputs(&mut self, ctx: &Context) {
        if self.folder_inputs[0][0].trim().is_empty() || self.mp3_input.trim().is_empty() {
            self.show_info("MP4 Folder 1 と MP3 Folder は必須です");
            return;
        }

        self.settings.mp4_folder_paths = self.folder_inputs.clone();
        self.settings.mp3_folder_path = self.mp3_input.clone();
        self.settings.volume = self.volume;
        self.settings.multiview_enabled = self.multiview_enabled_input;
        self.settings.multiview_video_size = self.multiview_video_size_input;
        self.settings.numpad_folder_switching = self.numpad_folder_switching_input;
        self.settings.normalize();

        let updated_preset = (!self.selected_preset.is_empty())
            .then(|| self.selected_preset.clone())
            .filter(|name| self.settings.presets.contains_key(name));
        if let Some(name) = &updated_preset {
            if let Some(preset) = self.settings.presets.get_mut(name) {
                preset.mp4_folder_paths = self.settings.mp4_folder_paths.clone();
                preset.mp3_folder_path = self.settings.mp3_folder_path.clone();
            }
        }
        self.settings.active_preset = updated_preset.clone();

        if let Err(error) = self.settings.save() {
            self.show_info(format!("設定保存失敗: {error}"));
            return;
        }

        self.active_preset = updated_preset;
        self.show_settings = false;
        self.reload_library(ctx);
        if self.active_preset.is_some() {
            self.show_info("設定とプリセットを保存しました");
        } else {
            self.show_info("設定を保存しました");
        }
    }
}

fn preset_hotkey_label(hotkey: Option<u8>) -> String {
    hotkey.map_or_else(|| "なし".to_owned(), |key| format!("F{key}"))
}
