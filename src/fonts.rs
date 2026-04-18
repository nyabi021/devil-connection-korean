use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

const KR_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSansKR-Regular.otf");
const KR_BOLD: &[u8] = include_bytes!("../assets/fonts/NotoSansKR-Bold.otf");
const JP_REGULAR: &[u8] = include_bytes!("../assets/fonts/NotoSansJP-Regular.otf");

pub const KR_REGULAR_KEY: &str = "NotoSansKR-Regular";
pub const KR_BOLD_KEY: &str = "NotoSansKR-Bold";
pub const JP_REGULAR_KEY: &str = "NotoSansJP-Regular";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        KR_REGULAR_KEY.to_string(),
        Arc::new(FontData::from_static(KR_REGULAR)),
    );
    fonts.font_data.insert(
        KR_BOLD_KEY.to_string(),
        Arc::new(FontData::from_static(KR_BOLD)),
    );
    fonts.font_data.insert(
        JP_REGULAR_KEY.to_string(),
        Arc::new(FontData::from_static(JP_REGULAR)),
    );

    let prop = fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default();
    prop.insert(0, KR_REGULAR_KEY.to_string());
    prop.insert(1, JP_REGULAR_KEY.to_string());

    let mono = fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default();
    mono.insert(0, KR_REGULAR_KEY.to_string());
    mono.insert(1, JP_REGULAR_KEY.to_string());

    fonts.families.insert(
        FontFamily::Name("bold".into()),
        vec![KR_BOLD_KEY.to_string(), JP_REGULAR_KEY.to_string()],
    );

    ctx.set_fonts(fonts);
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}
