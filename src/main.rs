#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod applog;
mod config;
mod icon;
mod loader;
mod media;
mod multiview;
mod playback;
mod ui;
mod updater;
mod win_icon;

use app::RndWalkerApp;

fn main() -> eframe::Result {
    applog::log(
        "startup",
        &format!("version={} exe={:?}", env!("CARGO_PKG_VERSION"), std::env::current_exe()),
    );

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 450.0])
            .with_icon(icon::app_icon()),
        ..Default::default()
    };

    let result = eframe::run_native(
        "rndWalker",
        options,
        Box::new(|cc| Ok(Box::new(RndWalkerApp::new(cc)))),
    );
    applog::log("shutdown", &format!("run_native -> {result:?}"));
    result
}
