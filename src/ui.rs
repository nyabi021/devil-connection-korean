//! egui-based installer UI with a Material-ish light theme.

use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use crate::config::{APP_TITLE, CREDITS, WINDOW_HEIGHT, WINDOW_WIDTH};
use crate::detect::{find_app_asar, find_game};
use crate::install::{Installer, Level, Message};

mod theme {
    use eframe::egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Style, Vec2, Visuals};

    pub const PRIMARY: Color32 = Color32::from_rgb(0x1a, 0x73, 0xe8);
    pub const PRIMARY_HOVER: Color32 = Color32::from_rgb(0x17, 0x62, 0xd2);
    pub const PRIMARY_ACTIVE: Color32 = Color32::from_rgb(0x13, 0x52, 0xb5);
    pub const PRIMARY_CONTAINER: Color32 = Color32::from_rgb(0xe8, 0xf0, 0xfe);
    pub const PRIMARY_CONTAINER_ACTIVE: Color32 = Color32::from_rgb(0xd2, 0xe3, 0xfc);
    pub const SURFACE: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
    pub const SURFACE_SUNKEN: Color32 = Color32::from_rgb(0xf8, 0xf9, 0xfa);
    pub const BACKGROUND: Color32 = Color32::from_rgb(0xf1, 0xf3, 0xf4);
    pub const ON_SURFACE: Color32 = Color32::from_rgb(0x20, 0x24, 0x2c);
    pub const ON_SURFACE_VARIANT: Color32 = Color32::from_rgb(0x5f, 0x63, 0x68);
    pub const OUTLINE: Color32 = Color32::from_rgb(0xda, 0xdc, 0xe0);
    pub const OUTLINE_SOFT: Color32 = Color32::from_rgb(0xe8, 0xea, 0xed);
    pub const DANGER: Color32 = Color32::from_rgb(0xd9, 0x3b, 0x3b);
    pub const DANGER_CONTAINER: Color32 = Color32::from_rgb(0xfc, 0xe8, 0xe8);

    pub fn card_shadow() -> Shadow {
        Shadow {
            offset: [0, 1],
            blur: 4,
            spread: 0,
            color: Color32::from_rgba_premultiplied(0, 0, 0, 18),
        }
    }

    pub fn apply(ctx: &eframe::egui::Context) {
        let mut v = Visuals::light();
        v.panel_fill = BACKGROUND;
        v.window_fill = SURFACE;
        v.extreme_bg_color = SURFACE;
        v.faint_bg_color = SURFACE_SUNKEN;
        v.window_stroke = Stroke::new(1.0, OUTLINE_SOFT);
        v.window_corner_radius = CornerRadius::same(16);
        v.menu_corner_radius = CornerRadius::same(12);
        v.window_shadow = Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_rgba_premultiplied(0, 0, 0, 36),
        };
        v.popup_shadow = card_shadow();
        v.override_text_color = Some(ON_SURFACE);
        v.hyperlink_color = PRIMARY;
        v.selection.bg_fill = PRIMARY_CONTAINER_ACTIVE;
        v.selection.stroke = Stroke::new(1.0, PRIMARY);

        v.widgets.noninteractive.bg_fill = SURFACE;
        v.widgets.noninteractive.weak_bg_fill = SURFACE;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, OUTLINE_SOFT);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ON_SURFACE);
        v.widgets.noninteractive.corner_radius = CornerRadius::same(8);

        v.widgets.inactive.bg_fill = SURFACE;
        v.widgets.inactive.weak_bg_fill = SURFACE;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, OUTLINE);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, ON_SURFACE);
        v.widgets.inactive.corner_radius = CornerRadius::same(20);
        v.widgets.inactive.expansion = 0.0;

        v.widgets.hovered.bg_fill = PRIMARY_CONTAINER;
        v.widgets.hovered.weak_bg_fill = PRIMARY_CONTAINER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, PRIMARY);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, PRIMARY);
        v.widgets.hovered.corner_radius = CornerRadius::same(20);
        v.widgets.hovered.expansion = 1.0;

        v.widgets.active.bg_fill = PRIMARY_CONTAINER_ACTIVE;
        v.widgets.active.weak_bg_fill = PRIMARY_CONTAINER_ACTIVE;
        v.widgets.active.bg_stroke = Stroke::new(1.0, PRIMARY_ACTIVE);
        v.widgets.active.fg_stroke = Stroke::new(1.0, PRIMARY_ACTIVE);
        v.widgets.active.corner_radius = CornerRadius::same(20);
        v.widgets.active.expansion = 1.0;

        v.widgets.open.bg_fill = PRIMARY_CONTAINER;
        v.widgets.open.weak_bg_fill = PRIMARY_CONTAINER;
        v.widgets.open.bg_stroke = Stroke::new(1.0, PRIMARY);
        v.widgets.open.fg_stroke = Stroke::new(1.0, PRIMARY);
        v.widgets.open.corner_radius = CornerRadius::same(20);

        ctx.set_visuals(v);

        let mut s: Style = (*ctx.global_style()).clone();
        s.spacing.item_spacing = Vec2::new(10.0, 8.0);
        s.spacing.button_padding = Vec2::new(16.0, 8.0);
        s.spacing.window_margin = Margin::same(18);
        s.spacing.menu_margin = Margin::same(8);
        s.spacing.interact_size = Vec2::new(40.0, 36.0);
        s.spacing.indent = 18.0;
        ctx.set_global_style(s);
    }
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::SURFACE)
        .inner_margin(egui::Margin::same(18))
        .corner_radius(egui::CornerRadius::same(14))
        .stroke(egui::Stroke::new(1.0, theme::OUTLINE_SOFT))
        .shadow(theme::card_shadow())
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .strong()
            .size(12.5)
            .color(theme::ON_SURFACE_VARIANT),
    );
}

fn primary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.scope(|ui| {
        {
            let w = &mut ui.visuals_mut().widgets;
            let no_stroke = egui::Stroke::NONE;
            let white = egui::Stroke::new(1.0, egui::Color32::WHITE);
            w.inactive.bg_fill = theme::PRIMARY;
            w.inactive.weak_bg_fill = theme::PRIMARY;
            w.inactive.bg_stroke = no_stroke;
            w.inactive.fg_stroke = white;
            w.inactive.corner_radius = egui::CornerRadius::same(20);
            w.hovered.bg_fill = theme::PRIMARY_HOVER;
            w.hovered.weak_bg_fill = theme::PRIMARY_HOVER;
            w.hovered.bg_stroke = no_stroke;
            w.hovered.fg_stroke = white;
            w.hovered.corner_radius = egui::CornerRadius::same(20);
            w.active.bg_fill = theme::PRIMARY_ACTIVE;
            w.active.weak_bg_fill = theme::PRIMARY_ACTIVE;
            w.active.bg_stroke = no_stroke;
            w.active.fg_stroke = white;
            w.active.corner_radius = egui::CornerRadius::same(20);
        }
        ui.add_enabled(
            enabled,
            egui::Button::new(
                egui::RichText::new(label)
                    .strong()
                    .color(egui::Color32::WHITE),
            )
            .min_size(egui::vec2(132.0, 38.0)),
        )
    })
    .inner
}

fn secondary_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(egui::vec2(92.0, 34.0)),
    )
}

fn danger_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.scope(|ui| {
        {
            let w = &mut ui.visuals_mut().widgets;
            w.inactive.bg_fill = theme::SURFACE;
            w.inactive.weak_bg_fill = theme::SURFACE;
            w.inactive.bg_stroke = egui::Stroke::new(1.0, theme::DANGER);
            w.inactive.fg_stroke = egui::Stroke::new(1.0, theme::DANGER);
            w.hovered.bg_fill = theme::DANGER_CONTAINER;
            w.hovered.weak_bg_fill = theme::DANGER_CONTAINER;
            w.hovered.bg_stroke = egui::Stroke::new(1.0, theme::DANGER);
            w.hovered.fg_stroke = egui::Stroke::new(1.0, theme::DANGER);
            w.active.bg_fill = egui::Color32::from_rgb(0xf6, 0xc6, 0xc6);
            w.active.weak_bg_fill = egui::Color32::from_rgb(0xf6, 0xc6, 0xc6);
            w.active.bg_stroke = egui::Stroke::new(1.0, theme::DANGER);
            w.active.fg_stroke = egui::Stroke::new(1.0, theme::DANGER);
        }
        ui.add(
            egui::Button::new(egui::RichText::new(label).color(theme::DANGER).strong())
                .min_size(egui::vec2(80.0, 34.0)),
        )
    })
    .inner
}

pub struct PatcherApp {
    path_input: String,
    path_valid: Option<bool>,
    log: Vec<LogLine>,
    progress: u8,
    installing: bool,
    cancel: Arc<AtomicBool>,
    channel: Option<Receiver<Message>>,
    detect_channel: Option<Receiver<Option<PathBuf>>>,
    dialog: Option<Dialog>,
}

struct LogLine {
    text: String,
    level: Level,
}

enum Dialog {
    Info { title: String, body: String },
    Error { title: String, body: String },
    ConfirmQuit,
}

impl PatcherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let mut app = Self {
            path_input: String::new(),
            path_valid: None,
            log: Vec::new(),
            progress: 0,
            installing: false,
            cancel: Arc::new(AtomicBool::new(false)),
            channel: None,
            detect_channel: None,
            dialog: None,
        };
        app.welcome();
        app
    }

    fn welcome(&mut self) {
        self.log_line(
            "메인 시나리오 번역 검수 'Ewan'님, 이미지 번역 '토니', '체퓨'님, 영상 번역 '민버드'님",
            Level::Success,
        );
        self.log_line("", Level::Info);
        self.log_line("'자동 감지' 버튼을 클릭하거나 게임 경로를 직접 선택해주세요.", Level::Info);
    }

    fn log_line(&mut self, text: impl Into<String>, level: Level) {
        self.log.push(LogLine { text: text.into(), level });
    }

    fn start_auto_detect(&mut self) {
        self.log_line("게임 경로를 자동으로 검색 중...", Level::Info);
        let (tx, rx) = channel();
        self.detect_channel = Some(rx);
        std::thread::spawn(move || {
            let found = find_game();
            let _ = tx.send(found);
        });
    }

    fn browse(&mut self) {
        let picker = rfd::FileDialog::new()
            .set_title("Devil Connection 게임 폴더를 선택하세요")
            .pick_folder();
        if let Some(path) = picker {
            self.set_path(path);
        }
    }

    fn set_path(&mut self, path: PathBuf) {
        let display = path.display().to_string();
        self.path_input = display.clone();
        let valid = find_app_asar(&path).is_some();
        self.path_valid = Some(valid);
        if valid {
            self.log_line(format!("게임 경로 선택: {display}"), Level::Success);
        } else {
            self.log_line(format!("게임 경로 선택: {display}"), Level::Info);
            self.log_line(
                "app.asar 파일을 찾을 수 없습니다. 올바른 게임 폴더인지 확인하세요.",
                Level::Warning,
            );
        }
    }

    fn start_install(&mut self) {
        let game_path = self.path_input.trim();
        if game_path.is_empty() {
            self.dialog = Some(Dialog::Error {
                title: "경로 없음".into(),
                body: "게임 경로를 먼저 선택해주세요.".into(),
            });
            return;
        }
        let path = PathBuf::from(game_path);
        if find_app_asar(&path).is_none() {
            self.path_valid = Some(false);
            self.dialog = Some(Dialog::Error {
                title: "잘못된 게임 경로".into(),
                body: "선택한 폴더에서 게임 파일(app.asar)을 찾을 수 없습니다.\n\n올바른 게임 설치 폴더를 선택해주세요.".into(),
            });
            return;
        }

        self.installing = true;
        self.progress = 0;
        self.cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel::<Message>();
        self.channel = Some(rx);
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            Installer {
                game_path: path,
                tx,
                cancel,
            }
            .run();
        });
    }

    fn cancel_install(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn guard_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.installing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !matches!(self.dialog, Some(Dialog::ConfirmQuit)) {
                self.dialog = Some(Dialog::ConfirmQuit);
            }
        }
    }

    fn pump_messages(&mut self, ctx: &egui::Context) {
        let mut finished: Option<(bool, String)> = None;
        if let Some(rx) = &self.channel {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    Message::Log { text, level } => self.log.push(LogLine { text, level }),
                    Message::Progress(p) => self.progress = p,
                    Message::Finished { success, message } => {
                        finished = Some((success, message));
                    }
                }
            }
        }
        if let Some((success, message)) = finished {
            self.installing = false;
            self.channel = None;
            if success {
                self.path_valid = None;
                self.dialog = Some(Dialog::Info {
                    title: "설치 완료".into(),
                    body: message,
                });
            } else {
                self.dialog = Some(Dialog::Error {
                    title: "설치 오류".into(),
                    body: message,
                });
            }
        }

        let mut detected: Option<Option<PathBuf>> = None;
        if let Some(rx) = &self.detect_channel {
            if let Ok(v) = rx.try_recv() {
                detected = Some(v);
            }
        }
        if let Some(found) = detected {
            self.detect_channel = None;
            match found {
                Some(path) => {
                    self.set_path(path);
                    self.log_line("게임을 찾았습니다!", Level::Success);
                }
                None => {
                    self.path_valid = None;
                    self.log_line("게임 경로를 자동으로 찾지 못했습니다.", Level::Warning);
                    self.log_line("'찾아보기' 버튼으로 직접 선택해주세요.", Level::Info);
                    self.dialog = Some(Dialog::Error {
                        title: "경로 감지 실패".into(),
                        body: "게임 경로를 자동으로 찾지 못했습니다.\n\n'찾아보기' 버튼을 눌러 직접 선택해주세요.".into(),
                    });
                }
            }
        }

        if self.installing || self.channel.is_some() || self.detect_channel.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }
}

impl eframe::App for PatcherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_messages(ctx);
        self.guard_close(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(20, 18)),
            )
            .show_inside(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("데빌 커넥션")
                            .size(30.0)
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("でびるコネクショん")
                            .size(12.0)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(CREDITS)
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.add_space(18.0);

                card_frame().show(ui, |ui| {
                    section_label(ui, "게임 경로");
                    ui.add_space(8.0);
                    let text_edit = egui::TextEdit::singleline(&mut self.path_input)
                        .hint_text("게임이 설치된 경로를 선택하세요")
                        .desired_width(ui.available_width())
                        .margin(egui::Margin::symmetric(10, 8));
                    ui.add(text_edit);
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let detect_enabled = !self.installing && self.detect_channel.is_none();
                        if secondary_button(ui, "자동 감지", detect_enabled).clicked() {
                            self.start_auto_detect();
                        }
                        if secondary_button(ui, "찾아보기", !self.installing).clicked() {
                            self.browse();
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.installing {
                                if danger_button(ui, "취소").clicked() {
                                    self.cancel_install();
                                }
                            } else if primary_button(ui, "설치 시작", true).clicked() {
                                self.start_install();
                            }
                        });
                    });
                });

                if self.installing || self.progress > 0 {
                    ui.add_space(14.0);
                    card_frame().show(ui, |ui| {
                        ui.horizontal(|ui| {
                            section_label(ui, "진행률");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!("{}%", self.progress))
                                            .size(12.0)
                                            .color(theme::ON_SURFACE_VARIANT),
                                    );
                                },
                            );
                        });
                        ui.add_space(10.0);
                        let pb = egui::ProgressBar::new(self.progress as f32 / 100.0)
                            .desired_width(ui.available_width())
                            .desired_height(6.0)
                            .corner_radius(egui::CornerRadius::same(6))
                            .fill(theme::PRIMARY)
                            .animate(self.installing);
                        ui.add(pb);
                    });
                }

                ui.add_space(14.0);
                card_frame().show(ui, |ui| {
                    section_label(ui, "설치 로그");
                    ui.add_space(8.0);
                    egui::Frame::new()
                        .fill(theme::SURFACE_SUNKEN)
                        .inner_margin(egui::Margin::same(12))
                        .corner_radius(egui::CornerRadius::same(10))
                        .stroke(egui::Stroke::new(1.0, theme::OUTLINE_SOFT))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .stick_to_bottom(true)
                                .auto_shrink([false, false])
                                .min_scrolled_height(220.0)
                                .max_height(260.0)
                                .show(ui, |ui| {
                                    for line in &self.log {
                                        if line.text.is_empty() {
                                            ui.add_space(4.0);
                                        } else {
                                            ui.colored_label(
                                                level_color(line.level),
                                                &line.text,
                                            );
                                        }
                                    }
                                });
                        });
                });

                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(
                            "본 프로그램은 ㈜넥슨코리아 메이플스토리 서체 및 ㈜우아한형제들 배달의민족 꾸불림체를 사용합니다.",
                        )
                        .size(11.5)
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });

        if let Some(dialog) = self.dialog.take() {
            self.show_dialog(ui.ctx(), dialog);
        }
    }
}

impl PatcherApp {
    fn show_dialog(&mut self, ctx: &egui::Context, dialog: Dialog) {
        let mut keep_open = true;
        let mut cancel_install = false;
        let mut force_quit = false;

        let title = match &dialog {
            Dialog::Info { title, .. } => title.clone(),
            Dialog::Error { title, .. } => title.clone(),
            Dialog::ConfirmQuit => "설치 중 종료".to_string(),
        };

        egui::Window::new(&title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(380.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .corner_radius(egui::CornerRadius::same(16))
                    .stroke(egui::Stroke::new(1.0, theme::OUTLINE_SOFT))
                    .shadow(egui::Shadow {
                        offset: [0, 12],
                        blur: 32,
                        spread: 0,
                        color: egui::Color32::from_rgba_premultiplied(0, 0, 0, 48),
                    })
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(340.0);
                match &dialog {
                    Dialog::Info { body, .. } => {
                        ui.label(egui::RichText::new(body).size(13.0));
                        ui.add_space(16.0);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if primary_button(ui, "확인", true).clicked() {
                                    keep_open = false;
                                }
                            },
                        );
                    }
                    Dialog::Error { body, .. } => {
                        ui.colored_label(theme::DANGER, egui::RichText::new(body).size(13.0));
                        ui.add_space(16.0);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if primary_button(ui, "확인", true).clicked() {
                                    keep_open = false;
                                }
                            },
                        );
                    }
                    Dialog::ConfirmQuit => {
                        ui.label(
                            egui::RichText::new(
                                "설치가 진행 중입니다.\n\n지금 종료하면 게임 파일이 손상될 수 있습니다. 정말로 종료하시겠습니까?",
                            )
                            .size(13.0),
                        );
                        ui.add_space(16.0);
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if danger_button(ui, "강제 종료").clicked() {
                                    force_quit = true;
                                    cancel_install = true;
                                    keep_open = false;
                                }
                                ui.add_space(8.0);
                                if secondary_button(ui, "계속 설치", true).clicked() {
                                    keep_open = false;
                                }
                            },
                        );
                    }
                }
            });

        if cancel_install {
            self.cancel.store(true, Ordering::Relaxed);
        }
        if force_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if keep_open {
            self.dialog = Some(dialog);
        }
    }
}

fn level_color(level: Level) -> egui::Color32 {
    match level {
        Level::Info => theme::ON_SURFACE,
        Level::Success => egui::Color32::from_rgb(0x1e, 0x8e, 0x3e),
        Level::Warning => egui::Color32::from_rgb(0xb0, 0x64, 0x00),
        Level::Error => theme::DANGER,
    }
}

fn install_fonts(ctx: &egui::Context) {
    const PRETENDARD: &[u8] = include_bytes!("../assets/Pretendard-Regular.otf");
    if PRETENDARD.is_empty() {
        return;
    }
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Pretendard".to_string(),
        Arc::new(egui::FontData::from_static(PRETENDARD)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "Pretendard".to_string());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("Pretendard".to_string());
    ctx.set_fonts(fonts);
}

pub fn window_size() -> egui::Vec2 {
    egui::vec2(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn title() -> &'static str {
    APP_TITLE
}
