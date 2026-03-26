import sys
from pathlib import Path

from PyQt6.QtCore import Qt
from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import (
    QFileDialog,
    QFrame,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QMainWindow,
    QMessageBox,
    QProgressBar,
    QPushButton,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from config import (
    APP_TITLE,
    BASE_PATH,
    CREDITS,
    VERSION,
    WINDOW_HEIGHT,
    WINDOW_WIDTH,
)
from installer import InstallWorker, find_app_asar
from updater import (
    UpdateChecker,
    UpdateDownloader,
    apply_self_update,
    extract_if_zip,
)
from utils import get_monospace_font, get_system_font

import platform

_SKIP_VERSION_FILE = Path.home() / ".devil-connection-patcher-skip"


def _load_skipped_version() -> str:
    try:
        return _SKIP_VERSION_FILE.read_text(encoding="utf-8").strip()
    except Exception:
        return ""


def _save_skipped_version(version: str) -> None:
    try:
        _SKIP_VERSION_FILE.write_text(version, encoding="utf-8")
    except Exception:
        pass


class KoreanPatchInstaller(QMainWindow):
    def __init__(self):
        super().__init__()
        self.worker: InstallWorker | None = None
        self.update_checker: UpdateChecker | None = None
        self.update_downloader: UpdateDownloader | None = None
        self._pending_download_url: str | None = None
        self._pending_latest_version: str | None = None
        self._init_ui()
        self._start_update_check()

    def _init_ui(self):
        self.setWindowTitle(APP_TITLE)
        self.setMinimumSize(WINDOW_WIDTH, WINDOW_HEIGHT)
        self.resize(WINDOW_WIDTH, WINDOW_HEIGHT)

        central = QWidget()
        self.setCentralWidget(central)

        root = QVBoxLayout()
        root.setContentsMargins(30, 30, 30, 30)
        root.setSpacing(20)
        central.setLayout(root)

        root.addLayout(self._build_title())
        root.addSpacing(10)
        root.addWidget(self._build_update_frame())
        root.addWidget(self._build_path_card())
        root.addWidget(self._build_progress_card())
        root.addWidget(self._build_log_card(), stretch=1)
        root.addWidget(self._build_footer())

        self._apply_styles()
        self._print_welcome()

    def _build_title(self) -> QVBoxLayout:
        layout = QVBoxLayout()
        layout.setSpacing(5)

        title = QLabel("でびるコネクショん")
        title.setFont(QFont(get_system_font(), 24, QFont.Weight.Bold))
        title.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(title)

        subtitle = QLabel("한글패치 프로그램")
        subtitle.setFont(QFont(get_system_font(), 11))
        subtitle.setAlignment(Qt.AlignmentFlag.AlignCenter)
        subtitle.setStyleSheet("color: #718096;")
        layout.addWidget(subtitle)

        credits = QLabel(CREDITS)
        credits.setFont(QFont(get_system_font(), 10))
        credits.setAlignment(Qt.AlignmentFlag.AlignCenter)
        credits.setStyleSheet("color: #4a5568; margin-top: 2px;")
        layout.addWidget(credits)

        return layout

    def _build_update_frame(self) -> QFrame:
        self.update_frame = QFrame()
        self.update_frame.setObjectName("update_frame")
        self.update_frame.setVisible(False)

        outer = QVBoxLayout()
        outer.setContentsMargins(16, 12, 16, 12)
        outer.setSpacing(8)

        top = QHBoxLayout()
        top.setSpacing(10)

        self.update_label = QLabel()
        self.update_label.setFont(QFont(get_system_font(), 10))
        self.update_label.setObjectName("update_label")
        top.addWidget(self.update_label, stretch=1)

        self.update_btn = QPushButton("업데이트 다운로드")
        self.update_btn.setObjectName("update_btn")
        self.update_btn.setFont(QFont(get_system_font(), 10, QFont.Weight.Bold))
        self.update_btn.setMinimumHeight(34)
        self.update_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.update_btn.clicked.connect(self._start_download)
        top.addWidget(self.update_btn)

        self.dismiss_btn = QPushButton("이 버전 건너뛰기")
        self.dismiss_btn.setObjectName("dismiss_btn")
        self.dismiss_btn.setFont(QFont(get_system_font(), 10))
        self.dismiss_btn.setMinimumHeight(34)
        self.dismiss_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        self.dismiss_btn.clicked.connect(self._dismiss_update)
        top.addWidget(self.dismiss_btn)

        outer.addLayout(top)

        self.update_progress = QProgressBar()
        self.update_progress.setObjectName("update_progress")
        self.update_progress.setMinimumHeight(6)
        self.update_progress.setTextVisible(False)
        self.update_progress.setRange(0, 100)
        self.update_progress.setVisible(False)
        outer.addWidget(self.update_progress)

        self.update_status_label = QLabel()
        self.update_status_label.setObjectName("update_status_label")
        self.update_status_label.setFont(QFont(get_system_font(), 9))
        self.update_status_label.setVisible(False)
        outer.addWidget(self.update_status_label)

        self.update_frame.setLayout(outer)
        return self.update_frame

    def _build_path_card(self) -> QFrame:
        card = self._card()
        layout = QVBoxLayout()
        layout.setContentsMargins(20, 20, 20, 20)
        layout.setSpacing(12)

        label = QLabel("게임 경로")
        label.setFont(QFont(get_system_font(), 10, QFont.Weight.Bold))
        layout.addWidget(label)

        self.path_input = QLineEdit()
        self.path_input.setFont(QFont(get_system_font(), 10))
        self.path_input.setPlaceholderText("게임이 설치된 경로를 선택하세요")
        self.path_input.setMinimumHeight(40)
        layout.addWidget(self.path_input)

        buttons = QHBoxLayout()
        buttons.setSpacing(8)

        self.auto_btn = self._button("자동 감지", self._auto_detect)
        self.browse_btn = self._button("찾아보기", self._browse)
        self.install_btn = self._button("설치 시작", self._start_installation)
        self.install_btn.setObjectName("install_btn")
        self.install_btn.setFont(QFont(get_system_font(), 11, QFont.Weight.Bold))

        buttons.addWidget(self.auto_btn)
        buttons.addWidget(self.browse_btn)
        buttons.addStretch()
        buttons.addWidget(self.install_btn)

        layout.addLayout(buttons)
        card.setLayout(layout)
        return card

    def _build_progress_card(self) -> QFrame:
        self.progress_card = self._card()
        layout = QVBoxLayout()
        layout.setContentsMargins(20, 15, 20, 15)

        self.progress_bar = QProgressBar()
        self.progress_bar.setMinimumHeight(8)
        self.progress_bar.setTextVisible(False)
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        layout.addWidget(self.progress_bar)

        self.progress_card.setLayout(layout)
        self.progress_card.setVisible(False)
        return self.progress_card

    def _build_log_card(self) -> QFrame:
        card = self._card()
        layout = QVBoxLayout()
        layout.setContentsMargins(20, 20, 20, 20)
        layout.setSpacing(10)

        label = QLabel("설치 로그")
        label.setFont(QFont(get_system_font(), 10, QFont.Weight.Bold))
        layout.addWidget(label)

        self.log_text = QTextEdit()
        self.log_text.setFont(QFont(get_monospace_font(), 9))
        self.log_text.setReadOnly(True)
        self.log_text.setMinimumHeight(250)
        layout.addWidget(self.log_text)

        card.setLayout(layout)
        return card

    def _build_footer(self) -> QLabel:
        footer = QLabel(
            "본 프로그램은 ㈜넥슨코리아 메이플스토리 서체 및 ㈜우아한형제들 배달의민족 꾸불림체를 사용합니다."
        )
        footer.setObjectName("footer_label")
        footer.setFont(QFont(get_system_font(), 8))
        footer.setAlignment(Qt.AlignmentFlag.AlignCenter)
        return footer

    def _card(self) -> QFrame:
        card = QFrame()
        card.setFrameShape(QFrame.Shape.StyledPanel)
        return card

    def _button(self, text: str, slot) -> QPushButton:
        btn = QPushButton(text)
        btn.setFont(QFont(get_system_font(), 10))
        btn.setMinimumHeight(40)
        btn.setCursor(Qt.CursorShape.PointingHandCursor)
        btn.clicked.connect(slot)
        return btn

    def _apply_styles(self):
        self.setStyleSheet("""
            QMainWindow { background-color: #f5f5f5; }
            QFrame {
                background-color: white;
                border: 1px solid #e2e8f0;
                border-radius: 8px;
            }
            QFrame#update_frame {
                background-color: #fffbeb;
                border: 1px solid #f6e05e;
            }
            QLabel { color: #2d3748; background: transparent; border: none; }
            QLabel#update_label { color: #744210; }
            QLabel#update_status_label { color: #744210; }
            QLabel#footer_label { color: #a0aec0; }
            QLineEdit {
                padding: 10px;
                border: 1px solid #e2e8f0;
                border-radius: 6px;
                background-color: white;
                color: #2d3748;
            }
            QLineEdit:focus { border: 1px solid #4a5568; }
            QPushButton {
                background-color: white;
                border: 1px solid #e2e8f0;
                border-radius: 6px;
                padding: 10px 20px;
                color: #2d3748;
            }
            QPushButton:hover { background-color: #f7fafc; border-color: #cbd5e0; }
            QPushButton:pressed { background-color: #edf2f7; }
            QPushButton:disabled { background-color: #f7fafc; color: #a0aec0; }
            QPushButton#install_btn {
                background-color: #48bb78; color: white; border: none;
                min-width: 160px;
            }
            QPushButton#install_btn:hover { background-color: #38a169; }
            QPushButton#install_btn:pressed { background-color: #2f855a; }
            QPushButton#install_btn:disabled { background-color: #c6f6d5; color: #68d391; }
            QPushButton#update_btn {
                background-color: #d69e2e; color: white; border: none; padding: 6px 16px;
            }
            QPushButton#update_btn:hover { background-color: #b7791f; }
            QPushButton#update_btn:disabled { background-color: #f6e05e; color: #744210; }
            QPushButton#dismiss_btn {
                background-color: transparent;
                border: 1px solid #d69e2e;
                color: #744210;
                padding: 6px 14px;
            }
            QPushButton#dismiss_btn:hover { background-color: #fefcbf; }
            QTextEdit {
                border: none;
                background-color: #fafafa;
                color: #2d3748;
                border-radius: 6px;
                padding: 10px;
            }
            QProgressBar {
                border: none;
                background-color: #e2e8f0;
                border-radius: 4px;
            }
            QProgressBar::chunk { background-color: #48bb78; border-radius: 4px; }
            QProgressBar#update_progress { background-color: #fefcbf; }
            QProgressBar#update_progress::chunk {
                background-color: #d69e2e; border-radius: 3px;
            }
        """)

    def _set_path_valid(self, valid: bool | None):
        if valid is None:
            self.path_input.setStyleSheet("")
        elif valid:
            self.path_input.setStyleSheet(
                "border: 2px solid #48bb78; border-radius: 6px;"
                "padding: 10px; background-color: white; color: #2d3748;"
            )
        else:
            self.path_input.setStyleSheet(
                "border: 2px solid #f6ad55; border-radius: 6px;"
                "padding: 10px; background-color: white; color: #2d3748;"
            )

    def _start_update_check(self):
        self.update_checker = UpdateChecker()
        self.update_checker.update_available.connect(self._on_update_available)
        self.update_checker.up_to_date.connect(
            lambda: self._log(f"현재 최신 버전입니다. (v{VERSION})", "info")
        )
        self.update_checker.check_failed.connect(lambda: None)
        self.update_checker.start()

    def _on_update_available(self, latest: str, url: str):
        if latest == _load_skipped_version():
            self._log(f"v{latest} 업데이트가 있지만 건너뛰기로 설정되어 있습니다.", "info")
            return
        self._pending_download_url = url
        self._pending_latest_version = latest
        self.update_label.setText(
            f"새 버전 <b>v{latest}</b>이 출시되었습니다!  (현재: v{VERSION})"
        )
        self.update_frame.setVisible(True)
        self._log(f"새 버전 v{latest}이 출시되었습니다! 업데이트 배너를 확인해주세요.", "warning")

    def _dismiss_update(self):
        if self._pending_latest_version:
            _save_skipped_version(self._pending_latest_version)
        self.update_frame.setVisible(False)

    def _start_download(self):
        if not self._pending_download_url:
            return

        self.update_btn.setEnabled(False)
        self.dismiss_btn.setEnabled(False)
        self.update_progress.setValue(0)
        self.update_progress.setVisible(True)
        self.update_status_label.setText("다운로드 중...")
        self.update_status_label.setVisible(True)

        self.update_downloader = UpdateDownloader(self._pending_download_url)
        self.update_downloader.progress.connect(self._on_download_progress)
        self.update_downloader.finished.connect(self._on_download_finished)
        self.update_downloader.failed.connect(self._on_download_failed)
        self.update_downloader.start()

    def _on_download_progress(self, percent: int):
        self.update_progress.setValue(percent)
        self.update_status_label.setText(f"다운로드 중... {percent}%")

    def _on_download_finished(self, file_path: str):
        self.update_progress.setValue(100)
        self.update_status_label.setText("다운로드 완료! 처리 중...")

        try:
            exe_path = extract_if_zip(file_path)
        except Exception as e:
            self._on_download_failed(f"압축 해제 실패: {e}")
            return

        is_frozen = getattr(sys, "frozen", False)
        if is_frozen:
            self.update_status_label.setText("다운로드 완료! 잠시 후 재시작됩니다...")
            reply = QMessageBox.question(
                self,
                "업데이트 적용",
                "업데이트를 적용하기 위해 패처를 재시작합니다.\n계속하시겠습니까?",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            )
            if reply == QMessageBox.StandardButton.Yes:
                apply_self_update(exe_path)
            else:
                self._reset_update_ui()
        else:
            QMessageBox.information(
                self,
                "업데이트 다운로드 완료",
                f"파일이 다운로드되었습니다:\n{exe_path}\n\n직접 실행하여 업데이트하세요.",
            )
            self._reset_update_ui()

    def _on_download_failed(self, error: str):
        self.update_status_label.setText(f"다운로드 실패: {error}")
        self.update_btn.setEnabled(True)
        self.dismiss_btn.setEnabled(True)
        self._log(f"업데이트 다운로드 실패: {error}", "error")

    def _reset_update_ui(self):
        self.update_btn.setEnabled(True)
        self.dismiss_btn.setEnabled(True)
        self.update_progress.setVisible(False)
        self.update_status_label.setVisible(False)

    def _browse(self):
        path = QFileDialog.getExistingDirectory(
            self, "Devil Connection 게임 폴더를 선택하세요", ""
        )
        if path:
            self.path_input.setText(path)
            valid = find_app_asar(Path(path)) is not None
            self._set_path_valid(valid)
            if valid:
                self._log(f"게임 경로 선택: {path}", "success")
            else:
                self._log(f"게임 경로 선택: {path}", "info")
                self._log("app.asar 파일을 찾을 수 없습니다. 올바른 게임 폴더인지 확인하세요.", "warning")

    def _auto_detect(self):
        self._log("게임 경로를 자동으로 검색 중...", "info")
        found = self._search_game_path()
        if found:
            self.path_input.setText(str(found))
            self._set_path_valid(True)
            self._log("게임을 찾았습니다!", "success")
            self._log(f"경로: {found}", "info")
        else:
            self._set_path_valid(None)
            self._log("게임 경로를 자동으로 찾지 못했습니다.", "warning")
            self._log("'찾아보기' 버튼으로 직접 선택해주세요.", "info")
            QMessageBox.warning(
                self,
                "경로 감지 실패",
                "게임 경로를 자동으로 찾지 못했습니다.\n\n'찾아보기' 버튼을 눌러 직접 선택해주세요.",
            )

    def _search_game_path(self) -> Path | None:
        game_dir = "でびるコネクショん"
        system = platform.system()

        candidates: list[Path] = []

        if system == "Windows":
            roots = [Path("C:/Program Files (x86)/Steam"), Path("C:/Program Files/Steam")]
            for drive in "DEFGHIJ":
                roots += [
                    Path(f"{drive}:/Steam"),
                    Path(f"{drive}:/Program Files (x86)/Steam"),
                    Path(f"{drive}:/Program Files/Steam"),
                    Path(f"{drive}:/SteamLibrary"),
                ]
            candidates = [r / "steamapps/common" / game_dir for r in roots]

        elif system == "Darwin":
            steam = Path.home() / "Library/Application Support/Steam"
            candidates = [steam / "steamapps/common" / game_dir]

        else:
            roots = [
                Path.home() / ".local/share/Steam",
                Path.home() / ".steam/steam",
            ]
            candidates = [r / "steamapps/common" / game_dir for r in roots]

        return next((p for p in candidates if p.exists()), None)

    def _start_installation(self):
        game_path = self.path_input.text().strip()
        if not game_path:
            QMessageBox.warning(self, "경로 없음", "게임 경로를 먼저 선택해주세요.")
            return

        if find_app_asar(Path(game_path)) is None:
            self._set_path_valid(False)
            QMessageBox.warning(
                self,
                "잘못된 게임 경로",
                "선택한 폴더에서 게임 파일(app.asar)을 찾을 수 없습니다.\n\n"
                "올바른 게임 설치 폴더를 선택해주세요.",
            )
            return

        for btn in (self.install_btn, self.auto_btn, self.browse_btn):
            btn.setEnabled(False)
        self.progress_bar.setValue(0)
        self.progress_card.setVisible(True)

        self.worker = InstallWorker(game_path, BASE_PATH)
        self.worker.log_signal.connect(self._log)
        self.worker.progress_signal.connect(self.progress_bar.setValue)
        self.worker.finished_signal.connect(self._on_install_finished)
        self.worker.start()

    def _on_install_finished(self, success: bool, message: str):
        for btn in (self.install_btn, self.auto_btn, self.browse_btn):
            btn.setEnabled(True)
        self.progress_card.setVisible(False)

        if success:
            self._set_path_valid(None)
            QMessageBox.information(self, "설치 완료", message)
        else:
            QMessageBox.critical(self, "설치 오류", message)

    def _log(self, message: str, level: str = "info"):
        colors = {
            "info": "#2d3748",
            "success": "#48bb78",
            "error": "#f56565",
            "warning": "#ed8936",
        }
        color = colors.get(level, "#2d3748")
        self.log_text.append(f'<span style="color: {color};">{message}</span>')
        self.log_text.verticalScrollBar().setValue(
            self.log_text.verticalScrollBar().maximum()
        )

    def _print_welcome(self):
        self._log("でびるコネクショん 한글패치 프로그램을 시작합니다.", "info")
        self._log("", "info")
        self._log(
            "메인 시나리오 번역 검수 'Ewan'님, 이미지 번역 '토니', '체퓨'님, "
            "영상 번역 '민버드'님께 진심으로 감사드립니다.",
            "success",
        )
        self._log("", "info")
        self._log("'자동 감지' 버튼을 클릭하거나 게임 경로를 직접 선택해주세요.", "info")

    def closeEvent(self, event):
        if self.worker and self.worker.isRunning():
            reply = QMessageBox.question(
                self,
                "설치 중",
                "설치가 진행 중입니다. 종료하시겠습니까?\n(취소 시 원본 파일이 자동으로 복원됩니다)",
                QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            )
            if reply == QMessageBox.StandardButton.Yes:
                self.worker.cancel()
                self.worker.wait()
                event.accept()
            else:
                event.ignore()
        else:
            event.accept()
