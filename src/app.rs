use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{Receiver, unbounded};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, ProgressBar, RichText,
    ScrollArea, Stroke, TextEdit, Vec2,
};

use crate::config::{CREDITS, FOOTER_TEXT};
use crate::fonts::bold_family;
use crate::installer::{self, InstallEvent, InstallHandle, LogLevel};
use crate::steam;

const COLOR_BG: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf5);
const COLOR_CARD: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
const COLOR_BORDER: Color32 = Color32::from_rgb(0xe2, 0xe8, 0xf0);
const COLOR_TEXT: Color32 = Color32::from_rgb(0x2d, 0x37, 0x48);
const COLOR_MUTED: Color32 = Color32::from_rgb(0x71, 0x80, 0x96);
const COLOR_MUTED_DARK: Color32 = Color32::from_rgb(0x4a, 0x55, 0x68);
const COLOR_FOOTER: Color32 = Color32::from_rgb(0xa0, 0xae, 0xc0);
const COLOR_ACCENT: Color32 = Color32::from_rgb(0x48, 0xbb, 0x78);
const COLOR_ACCENT_HOVER: Color32 = Color32::from_rgb(0x38, 0xa1, 0x69);
const COLOR_SUCCESS: Color32 = Color32::from_rgb(0x48, 0xbb, 0x78);
const COLOR_WARNING: Color32 = Color32::from_rgb(0xed, 0x89, 0x36);
const COLOR_ERROR: Color32 = Color32::from_rgb(0xf5, 0x65, 0x65);
const COLOR_PATH_VALID: Color32 = Color32::from_rgb(0x48, 0xbb, 0x78);
const COLOR_PATH_INVALID: Color32 = Color32::from_rgb(0xf6, 0xad, 0x55);
const COLOR_LOG_BG: Color32 = Color32::from_rgb(0xfa, 0xfa, 0xfa);

struct LogEntry {
    level: LogLevel,
    msg: String,
}

enum Modal {
    Info { title: String, body: String },
    Warning { title: String, body: String },
    Error { title: String, body: String },
    CloseConfirm,
}

pub struct App {
    ctx: egui::Context,
    game_path: String,
    path_valid: Option<bool>,

    log: Vec<LogEntry>,
    progress: u32,
    show_progress: bool,

    install_rx: Option<Receiver<InstallEvent>>,
    install_handle: Option<InstallHandle>,

    detect_rx: Option<Receiver<Option<PathBuf>>>,
    detecting: bool,

    modal: Option<Modal>,
    allow_close: bool,
}

impl App {
    pub fn new(ctx: egui::Context) -> Self {
        let mut s = Self {
            ctx,
            game_path: String::new(),
            path_valid: None,
            log: Vec::new(),
            progress: 0,
            show_progress: false,
            install_rx: None,
            install_handle: None,
            detect_rx: None,
            detecting: false,
            modal: None,
            allow_close: false,
        };
        s.print_welcome();
        s
    }

    fn print_welcome(&mut self) {
        self.push_log(LogLevel::Info, "데빌 커넥션 한글패치를 시작합니다.");
        self.push_log(LogLevel::Info, "");
        self.push_log(
            LogLevel::Success,
            "메인 시나리오 번역 검수 'Ewan'님, 이미지 번역 '토니', '체퓨'님, 영상 번역 '민버드'님께 진심으로 감사드립니다.",
        );
        self.push_log(LogLevel::Info, "");
        self.push_log(
            LogLevel::Info,
            "'자동 감지' 버튼을 클릭하거나 게임 경로를 직접 선택해주세요.",
        );
    }

    fn push_log(&mut self, level: LogLevel, msg: impl Into<String>) {
        self.log.push(LogEntry {
            level,
            msg: msg.into(),
        });
    }

    fn is_installing(&self) -> bool {
        self.install_handle.is_some()
    }

    fn drain_install_events(&mut self) {
        let Some(rx) = &self.install_rx else { return };
        let rx = rx.clone();
        loop {
            match rx.try_recv() {
                Ok(ev) => self.handle_install_event(ev),
                Err(_) => break,
            }
        }
    }

    fn handle_install_event(&mut self, ev: InstallEvent) {
        match ev {
            InstallEvent::Log(level, msg) => self.push_log(level, msg),
            InstallEvent::Progress(p) => self.progress = p,
            InstallEvent::Finished { success, message } => {
                self.show_progress = false;
                if let Some(mut h) = self.install_handle.take() {
                    h.join();
                }
                self.install_rx = None;
                if success {
                    self.path_valid = None;
                    self.modal = Some(Modal::Info {
                        title: "설치 완료".into(),
                        body: message,
                    });
                } else {
                    self.modal = Some(Modal::Error {
                        title: "설치 오류".into(),
                        body: message,
                    });
                }
            }
        }
    }

    fn drain_detect_events(&mut self) {
        let Some(rx) = &self.detect_rx else { return };
        if let Ok(result) = rx.try_recv() {
            self.detecting = false;
            self.detect_rx = None;
            match result {
                Some(found) => {
                    self.game_path = found.display().to_string();
                    self.path_valid = Some(true);
                    self.push_log(LogLevel::Success, "게임을 찾았습니다!");
                    self.push_log(LogLevel::Info, format!("경로: {}", found.display()));
                }
                None => {
                    self.path_valid = None;
                    self.push_log(LogLevel::Warning, "게임 경로를 자동으로 찾지 못했습니다.");
                    self.push_log(LogLevel::Info, "'찾아보기' 버튼으로 직접 선택해주세요.");
                    self.modal = Some(Modal::Warning {
                        title: "경로 감지 실패".into(),
                        body: "게임 경로를 자동으로 찾지 못했습니다.\n\n'찾아보기' 버튼을 눌러 직접 선택해주세요.".into(),
                    });
                }
            }
        }
    }

    fn start_detect(&mut self) {
        if self.detecting || self.is_installing() {
            return;
        }
        self.push_log(LogLevel::Info, "게임 경로를 자동으로 검색 중...");
        self.detecting = true;
        let (tx, rx) = unbounded::<Option<PathBuf>>();
        self.detect_rx = Some(rx);
        let ctx = self.ctx.clone();
        thread::spawn(move || {
            let found = steam::auto_detect();
            let _ = tx.send(found);
            ctx.request_repaint();
        });
    }

    fn start_browse(&mut self) {
        if self.is_installing() {
            return;
        }
        if let Some(picked) = rfd::FileDialog::new()
            .set_title("Devil Connection 게임 폴더를 선택하세요")
            .pick_folder()
        {
            let path_str = picked.display().to_string();
            self.game_path = path_str.clone();
            let valid = steam::find_app_asar(&picked).is_some();
            self.path_valid = Some(valid);
            if valid {
                self.push_log(LogLevel::Success, format!("게임 경로 선택: {}", path_str));
            } else {
                self.push_log(LogLevel::Info, format!("게임 경로 선택: {}", path_str));
                self.push_log(
                    LogLevel::Warning,
                    "app.asar 파일을 찾을 수 없습니다. 올바른 게임 폴더인지 확인하세요.",
                );
            }
        }
    }

    fn start_install(&mut self) {
        if self.is_installing() {
            return;
        }
        let path = self.game_path.trim();
        if path.is_empty() {
            self.modal = Some(Modal::Warning {
                title: "경로 없음".into(),
                body: "게임 경로를 먼저 선택해주세요.".into(),
            });
            return;
        }
        let path_buf = PathBuf::from(path);
        if steam::find_app_asar(&path_buf).is_none() {
            self.path_valid = Some(false);
            self.modal = Some(Modal::Warning {
                title: "잘못된 게임 경로".into(),
                body: "선택한 폴더에서 게임 파일(app.asar)을 찾을 수 없습니다.\n\n올바른 게임 설치 폴더를 선택해주세요.".into(),
            });
            return;
        }

        self.progress = 0;
        self.show_progress = true;
        let (tx, rx) = unbounded::<InstallEvent>();
        self.install_rx = Some(rx);
        self.install_handle = Some(installer::spawn(path_buf, tx));
    }

    fn try_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.allow_close {
            return;
        }
        if self.is_installing() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !matches!(self.modal, Some(Modal::CloseConfirm)) {
                self.modal = Some(Modal::CloseConfirm);
            }
        }
    }

    fn render_title(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new("데빌 커넥션 한글패치")
                    .font(FontId::new(26.0, bold_family()))
                    .color(COLOR_TEXT),
            );
            ui.add_space(2.0);
            ui.label(
                RichText::new("でびるコネクショん")
                    .size(12.0)
                    .color(COLOR_MUTED),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(CREDITS)
                    .size(11.0)
                    .color(COLOR_MUTED_DARK),
            );
        });
    }

    fn render_path_card(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label(
                RichText::new("게임 경로")
                    .font(FontId::new(12.0, bold_family()))
                    .color(COLOR_TEXT),
            );
            ui.add_space(8.0);

            let stroke = match self.path_valid {
                Some(true) => Stroke::new(2.0, COLOR_PATH_VALID),
                Some(false) => Stroke::new(2.0, COLOR_PATH_INVALID),
                None => Stroke::new(1.0, COLOR_BORDER),
            };
            Frame::NONE
                .fill(Color32::WHITE)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    let response = ui.add(
                        TextEdit::singleline(&mut self.game_path)
                            .desired_width(f32::INFINITY)
                            .hint_text("게임이 설치된 경로를 선택하세요")
                            .frame(Frame::NONE)
                            .text_color(COLOR_TEXT),
                    );
                    if response.changed() {
                        let p = PathBuf::from(self.game_path.trim());
                        self.path_valid = if self.game_path.trim().is_empty() {
                            None
                        } else {
                            Some(steam::find_app_asar(&p).is_some())
                        };
                    }
                });

            ui.add_space(12.0);

            let disabled = self.is_installing() || self.detecting;
            ui.horizontal(|ui| {
                if styled_button(ui, "자동 감지", false, disabled).clicked() {
                    self.start_detect();
                }
                if styled_button(ui, "찾아보기", false, disabled).clicked() {
                    self.start_browse();
                }
                ui.add_space(ui.available_width() - 180.0);
                if styled_button(ui, "설치 시작", true, disabled).clicked() {
                    self.start_install();
                }
            });
        });
    }

    fn render_progress_card(&self, ui: &mut egui::Ui) {
        if !self.show_progress {
            return;
        }
        card(ui, |ui| {
            let bar = ProgressBar::new(self.progress as f32 / 100.0)
                .desired_width(ui.available_width())
                .corner_radius(CornerRadius::same(4))
                .fill(COLOR_ACCENT)
                .show_percentage();
            ui.add(bar);
        });
    }

    fn render_log_card(&self, ui: &mut egui::Ui) {
        let full_height = ui.available_height();
        card_sized(ui, full_height, |ui| {
            ui.label(
                RichText::new("설치 로그")
                    .font(FontId::new(12.0, bold_family()))
                    .color(COLOR_TEXT),
            );
            ui.add_space(8.0);

            Frame::NONE
                .fill(COLOR_LOG_BG)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            for entry in &self.log {
                                let color = match entry.level {
                                    LogLevel::Info => COLOR_TEXT,
                                    LogLevel::Success => COLOR_SUCCESS,
                                    LogLevel::Warning => COLOR_WARNING,
                                    LogLevel::Error => COLOR_ERROR,
                                };
                                if entry.msg.is_empty() {
                                    ui.add_space(4.0);
                                } else {
                                    ui.label(
                                        RichText::new(&entry.msg)
                                            .color(color)
                                            .size(11.0)
                                            .monospace(),
                                    );
                                }
                            }
                        });
                });
        });
    }

    fn render_footer(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(FOOTER_TEXT)
                    .size(9.5)
                    .color(COLOR_FOOTER),
            );
        });
    }

    fn render_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = &self.modal else { return };
        let (title, body, kind): (String, String, ModalKind) = match modal {
            Modal::Info { title, body } => (title.clone(), body.clone(), ModalKind::Info),
            Modal::Warning { title, body } => (title.clone(), body.clone(), ModalKind::Warning),
            Modal::Error { title, body } => (title.clone(), body.clone(), ModalKind::Error),
            Modal::CloseConfirm => (
                "설치 중".into(),
                "설치가 진행 중입니다. 종료하시겠습니까?\n(취소 시 원본 파일이 자동으로 복원됩니다)".into(),
                ModalKind::Confirm,
            ),
        };

        let mut dismiss = false;
        let mut confirm = false;
        egui::Modal::new(egui::Id::new("modal_dialog")).show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(
                RichText::new(&title)
                    .font(FontId::new(14.0, bold_family()))
                    .color(COLOR_TEXT),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(&body).color(COLOR_TEXT).size(11.0));
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match kind {
                        ModalKind::Confirm => {
                            if styled_button(ui, "아니오", false, false).clicked() {
                                dismiss = true;
                            }
                            if styled_button(ui, "예", true, false).clicked() {
                                confirm = true;
                            }
                        }
                        _ => {
                            if styled_button(ui, "확인", true, false).clicked() {
                                dismiss = true;
                            }
                        }
                    }
                });
            });
        });

        if confirm {
            if let Some(h) = &self.install_handle {
                h.cancel();
            }
            if let Some(mut h) = self.install_handle.take() {
                h.join();
            }
            self.install_rx = None;
            self.allow_close = true;
            self.modal = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if dismiss {
            self.modal = None;
        }
    }
}

enum ModalKind {
    Info,
    Warning,
    Error,
    Confirm,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.drain_install_events();
        self.drain_detect_events();

        egui::Panel::bottom("footer_panel")
            .frame(
                Frame::NONE.fill(COLOR_BG).inner_margin(Margin {
                    left: 24,
                    right: 24,
                    top: 8,
                    bottom: 16,
                }),
            )
            .show_separator_line(false)
            .resizable(false)
            .show_inside(ui, |ui| {
                self.render_footer(ui);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .fill(COLOR_BG)
                    .inner_margin(Margin {
                        left: 24,
                        right: 24,
                        top: 24,
                        bottom: 0,
                    }),
            )
            .show_inside(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 12.0);
                self.render_title(ui);
                ui.add_space(8.0);
                self.render_path_card(ui);
                self.render_progress_card(ui);
                self.render_log_card(ui);
            });

        self.render_modal(&ctx);
        self.try_close(&ctx);

        if self.is_installing() || self.detecting {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }
}

fn card_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 3],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(22),
    }
}

fn card(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    Frame::NONE
        .fill(COLOR_CARD)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(20))
        .shadow(card_shadow())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui);
        });
}

fn card_sized(ui: &mut egui::Ui, height: f32, contents: impl FnOnce(&mut egui::Ui)) {
    Frame::NONE
        .fill(COLOR_CARD)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(20))
        .shadow(card_shadow())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(height.max(120.0));
            contents(ui);
        });
}

fn styled_button(ui: &mut egui::Ui, label: &str, primary: bool, disabled: bool) -> egui::Response {
    let (bg, text_color, hover) = if primary {
        if disabled {
            (
                Color32::from_rgb(0xc6, 0xf6, 0xd5),
                Color32::from_rgb(0x68, 0xd3, 0x91),
                Color32::from_rgb(0xc6, 0xf6, 0xd5),
            )
        } else {
            (COLOR_ACCENT, Color32::WHITE, COLOR_ACCENT_HOVER)
        }
    } else if disabled {
        (
            Color32::from_rgb(0xf7, 0xfa, 0xfc),
            Color32::from_rgb(0xa0, 0xae, 0xc0),
            Color32::from_rgb(0xf7, 0xfa, 0xfc),
        )
    } else {
        (Color32::WHITE, COLOR_TEXT, Color32::from_rgb(0xf7, 0xfa, 0xfc))
    };

    let button = egui::Button::new(
        RichText::new(label)
            .color(text_color)
            .font(FontId::new(12.0, if primary { bold_family() } else { egui::FontFamily::Proportional }))
            .size(12.0),
    )
    .fill(bg)
    .stroke(if primary {
        Stroke::NONE
    } else {
        Stroke::new(1.0, COLOR_BORDER)
    })
    .corner_radius(CornerRadius::same(6))
    .min_size(if primary { Vec2::new(140.0, 34.0) } else { Vec2::new(90.0, 34.0) });

    let resp = ui.add_enabled(!disabled, button);
    let _ = hover;
    if !disabled {
        resp.clone().on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}
