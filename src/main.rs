#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod asar;
mod config;
mod fonts;
mod installer;
mod steam;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let viewport = egui::ViewportBuilder::default()
        .with_title(config::APP_TITLE)
        .with_inner_size([config::WINDOW_WIDTH, config::WINDOW_HEIGHT])
        .with_min_inner_size([640.0, 720.0])
        .with_icon(load_icon());

    let native_options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        config::APP_TITLE,
        native_options,
        Box::new(|cc| {
            fonts::install(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc.egui_ctx.clone())))
        }),
    )
}

fn load_icon() -> egui::IconData {
    const PNG: &[u8] = include_bytes!("../icons/icon.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(PNG));
    match decoder.read_info() {
        Ok(mut reader) => {
            let size = match reader.output_buffer_size() {
                Some(s) => s,
                None => return egui::IconData::default(),
            };
            let mut buf = vec![0; size];
            let info = match reader.next_frame(&mut buf) {
                Ok(info) => info,
                Err(_) => return egui::IconData::default(),
            };
            buf.truncate(info.buffer_size());
            let rgba = to_rgba(&buf, info.color_type);
            egui::IconData {
                rgba,
                width: info.width,
                height: info.height,
            }
        }
        Err(_) => egui::IconData::default(),
    }
}

fn to_rgba(buf: &[u8], color_type: png::ColorType) -> Vec<u8> {
    match color_type {
        png::ColorType::Rgba => buf.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            for px in buf.chunks_exact(3) {
                out.extend_from_slice(px);
                out.push(0xff);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity(buf.len() * 2);
            for px in buf.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity(buf.len() * 4);
            for &g in buf {
                out.extend_from_slice(&[g, g, g, 0xff]);
            }
            out
        }
        _ => buf.to_vec(),
    }
}
