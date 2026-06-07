pub fn app_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/rndwalker-256.png"))
        .expect("embedded application icon must be a valid PNG")
}
