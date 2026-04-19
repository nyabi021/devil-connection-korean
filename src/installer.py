import platform
import shutil
from pathlib import Path

from PyQt6.QtCore import QThread, pyqtSignal

from config import PATCH_DIRS


class InstallCancelledError(Exception):
    pass


def find_app_asar(game_path: Path) -> Path | None:
    system = platform.system()
    if system == "Darwin":
        asar_path = game_path / "DevilConnection.app/Contents/Resources/app.asar"
    else:
        asar_path = game_path / "resources/app.asar"
    return asar_path if asar_path.exists() else None


class InstallWorker(QThread):
    log_signal = pyqtSignal(str, str)
    progress_signal = pyqtSignal(int)
    finished_signal = pyqtSignal(bool, str)

    def __init__(self, game_path: str, base_path: Path):
        super().__init__()
        self.game_path = Path(game_path)
        self.base_path = base_path
        self._cancel_flag = False

    def cancel(self):
        self._cancel_flag = True

    def run(self):
        asar_path: Path | None = None
        backup_path: Path | None = None
        app_folder: Path | None = None

        try:
            self.log_signal.emit("=" * 60, "info")
            self.log_signal.emit("설치를 시작합니다...", "info")
            self.log_signal.emit("1단계: app.asar 파일 찾기...", "info")

            asar_path = find_app_asar(self.game_path)
            if not asar_path:
                raise Exception("app.asar 파일을 찾을 수 없습니다. 게임 경로를 확인해주세요.")

            self.log_signal.emit(f"app.asar 파일 위치: {asar_path}", "success")
            self.progress_signal.emit(5)
            self._check_cancel()

            resources_dir = asar_path.parent
            app_folder = resources_dir / "app"
            backup_path = resources_dir / "app.asar.backup"

            self.log_signal.emit("2단계: 원본 파일 백업...", "info")
            if backup_path.exists():
                self.log_signal.emit("백업 파일이 이미 존재합니다. 기존 백업을 유지합니다.", "info")
            else:
                shutil.copy2(asar_path, backup_path)
                self.log_signal.emit("백업 완료", "success")
            self.progress_signal.emit(15)
            self._check_cancel()

            self.log_signal.emit("3단계: 기존 패치 파일 정리...", "info")
            if app_folder.exists():
                self.log_signal.emit("기존 app 폴더를 삭제합니다...", "info")
                shutil.rmtree(app_folder)
                self.log_signal.emit("삭제 완료", "success")
            self.progress_signal.emit(20)
            self._check_cancel()

            self.log_signal.emit("4단계: app.asar 압축 해제 중... (시간이 걸릴 수 있습니다)", "info")
            from asar import extract_archive

            extract_archive(asar_path, app_folder)
            self.log_signal.emit("압축 해제 완료", "success")
            self.progress_signal.emit(40)
            self._check_cancel()

            self.log_signal.emit("5단계: 번역 파일 복사 중...", "info")
            valid_dirs = [d for d in PATCH_DIRS if (self.base_path / d).exists()]
            total = max(len(valid_dirs), 1)
            for i, dir_name in enumerate(valid_dirs):
                self._check_cancel()
                src = self.base_path / dir_name
                dst = app_folder / dir_name
                self.log_signal.emit(f"  - {dir_name} 폴더 복사 중...", "info")
                shutil.copytree(src, dst, dirs_exist_ok=True)
                self.log_signal.emit(f"  - {dir_name} 복사 완료", "success")
                self.progress_signal.emit(min(40 + int((i + 1) / total * 40), 80))

            self._check_cancel()

            self.log_signal.emit("6단계: app 폴더를 app.asar로 재압축 중... (시간이 걸릴 수 있습니다)", "info")
            if asar_path.exists() and asar_path.is_file():
                asar_path.unlink()
                self.log_signal.emit("원본 app.asar 파일을 삭제했습니다.", "info")

            from asar import create_archive

            create_archive(app_folder, asar_path, unpack="*.node")
            self.log_signal.emit("app.asar 재압축 완료", "success")
            self.progress_signal.emit(90)

            self.log_signal.emit("7단계: 임시 파일 정리 중...", "info")
            if app_folder.exists():
                shutil.rmtree(app_folder)
                self.log_signal.emit("app 폴더를 삭제했습니다.", "success")
            self.progress_signal.emit(100)

            self.log_signal.emit("=" * 60, "info")
            self.log_signal.emit("한글패치가 완료되었습니다!", "success")
            self.log_signal.emit("Steam에서 게임을 실행하면 한글로 플레이하실 수 있습니다.", "success")

            if platform.system() == "Darwin":
                self.log_signal.emit("", "info")
                self.log_signal.emit("macOS 사용자 안내:", "warning")
                self.log_signal.emit("게임 실행 시 '손상되었습니다' 경고가 나타날 수 있습니다.", "info")
                self.log_signal.emit("이는 정상적인 macOS 보안 경고이며, 다음과 같이 해결하세요:", "info")
                self.log_signal.emit("1. 시스템 설정 > 개인정보 보호 및 보안 열기", "info")
                self.log_signal.emit("2. '그래도 열기' 버튼 클릭", "info")

            self.log_signal.emit("=" * 60, "info")
            self.finished_signal.emit(True, self._complete_message())

        except InstallCancelledError:
            self.log_signal.emit("설치가 취소되었습니다. 원본 파일을 복원 중...", "warning")
            self._restore_backup(asar_path, backup_path, app_folder)
            self.finished_signal.emit(False, "설치가 취소되었습니다.\n원본 파일이 복원되었습니다.")

        except Exception as e:
            self.log_signal.emit("=" * 60, "error")
            self.log_signal.emit(f"설치 중 오류 발생: {str(e)}", "error")
            self.log_signal.emit("=" * 60, "error")
            self.finished_signal.emit(False, f"설치 중 오류가 발생했습니다:\n\n{str(e)}")

    def _check_cancel(self):
        if self._cancel_flag:
            raise InstallCancelledError("사용자에 의해 취소되었습니다.")

    def _restore_backup(
        self,
        asar_path: Path | None,
        backup_path: Path | None,
        app_folder: Path | None,
    ):
        try:
            if app_folder and app_folder.exists():
                shutil.rmtree(app_folder)
            if backup_path and backup_path.exists() and asar_path:
                shutil.copy2(backup_path, asar_path)
                self.log_signal.emit("원본 파일 복원 완료", "success")
        except Exception as e:
            self.log_signal.emit(f"복원 중 오류: {e}", "error")

    def _complete_message(self) -> str:
        if platform.system() == "Darwin":
            return (
                "한글패치가 완료되었습니다!\n\n"
                "Steam에서 게임을 실행하시면 됩니다.\n\n"
                "'손상되었습니다' 경고가 나타나면:\n"
                "시스템 설정 > 개인정보 보호 및 보안\n"
                "에서 '그래도 열기' 버튼을 클릭하세요."
            )
        return (
            "한글패치가 완료되었습니다!\n\n"
            "Steam에서 게임을 실행하면 한글로 플레이하실 수 있습니다."
        )
