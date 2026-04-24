#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod asar;
mod config;
mod detect;
mod install;
mod ui;

fn main() -> eframe::Result<()> {
    let size = ui::window_size();
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(ui::title())
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_icon(load_icon()),
        ..Default::default()
    };
    eframe::run_native(
        ui::title(),
        native_options,
        Box::new(|cc| Ok(Box::new(ui::PatcherApp::new(cc)))),
    )
}

fn load_icon() -> eframe::egui::IconData {
    const PNG: &[u8] = include_bytes!("../assets/icon.png");
    match image::load_from_memory_with_format(PNG, image::ImageFormat::Png) {
        Ok(img) => {
            let rgba = img.into_rgba8();
            let (width, height) = rgba.dimensions();
            eframe::egui::IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            }
        }
        Err(_) => eframe::egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        },
    }
}
