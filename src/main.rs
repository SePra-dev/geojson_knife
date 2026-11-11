#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod io;
mod model;
mod qml;
mod ui;

fn main() -> eframe::Result<()> {
    let icon = egui::IconData {
        rgba: include_bytes!("../assets/icon.rgba").to_vec(),
        width: 32,
        height: 32,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0])
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };
    eframe::run_native(
        "GeoJSON Toolkit",
        options,
        Box::new(|cc| Box::new(ui::GeoJsonToolkitApp::new(cc))),
    )
}
