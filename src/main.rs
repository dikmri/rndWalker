#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod icon;
mod loader;
mod media;
mod multiview;
mod playback;
mod ui;
mod updater;

use app::RndWalkerApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 450.0])
            .with_icon(icon::app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "rndWalker",
        options,
        Box::new(|cc| Ok(Box::new(RndWalkerApp::new(cc)))),
    )
}
