import json
import os
import platform
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path
from PyQt6.QtCore import QThread, pyqtSignal
from PyQt6.QtWidgets import QApplication

from config import (
    VERSION,
    GITHUB_OWNER,
    GITHUB_REPO,
    GITHUB_API_LATEST,
    GITHUB_RELEASES_URL,
)

_USER_AGENT = f"{GITHUB_REPO}-patcher/{VERSION}"

class UpdateChecker(QThread):
    update_available = pyqtSignal(str, str)  
    up_to_date = pyqtSignal()
    check_failed = pyqtSignal()

    def run(self):
        try:
            req = urllib.request.Request(
                GITHUB_API_LATEST,
                headers={"User-Agent": _USER_AGENT},
            )
            with urllib.request.urlopen(req, timeout=8) as response:
                data = json.loads(response.read().decode())

            latest_tag = data.get("tag_name", "").lstrip("v")
            if not latest_tag:
                self.check_failed.emit()
                return

            if _is_newer(latest_tag, VERSION):
                url = _find_asset_url(data.get("assets", []))
                self.update_available.emit(latest_tag, url or GITHUB_RELEASES_URL)
            else:
                self.up_to_date.emit()

        except Exception:
            self.check_failed.emit()

class UpdateDownloader(QThread):
    progress = pyqtSignal(int)  
    finished = pyqtSignal(str)  
    failed = pyqtSignal(str)   

    def __init__(self, url: str):
        super().__init__()
        self.url = url

    def run(self):
        try:
            req = urllib.request.Request(
                self.url,
                headers={"User-Agent": _USER_AGENT},
            )
            suffix = Path(self.url).suffix or ""
            tmp = tempfile.NamedTemporaryFile(delete=False, suffix=suffix)

            with urllib.request.urlopen(req, timeout=60) as response:
                total = int(response.headers.get("Content-Length", 0))
                downloaded = 0

                while chunk := response.read(8192):
                    tmp.write(chunk)
                    downloaded += len(chunk)
                    if total > 0:
                        self.progress.emit(int(downloaded * 100 / total))

            tmp.close()
            self.finished.emit(tmp.name)

        except Exception as e:
            self.failed.emit(str(e))

def extract_if_zip(file_path: str) -> str:
    if not file_path.endswith(".zip"):
        return file_path

    extract_dir = file_path + "_extracted"
    with zipfile.ZipFile(file_path, "r") as zf:
        zf.extractall(extract_dir)

    os.unlink(file_path)

    candidates = list(Path(extract_dir).iterdir())
    if not candidates:
        raise RuntimeError("zip 파일 내부가 비어 있습니다.")

    exe = candidates[0]
    if platform.system() != "Windows":
        os.chmod(exe, 0o755)

    return str(exe)

def apply_self_update(new_file_path: str) -> None:
    is_frozen = getattr(sys, "frozen", False)
    current_exe = Path(sys.executable if is_frozen else sys.argv[0]).resolve()
    new_file = Path(new_file_path).resolve()

    if platform.system() == "Windows":
        bat = tempfile.NamedTemporaryFile(
            delete=False, suffix=".bat", mode="w", encoding="utf-8"
        )
        bat.write(
            f"@echo off\n"
            f"ping 127.0.0.1 -n 3 > nul\n"
            f"copy /Y \"{new_file}\" \"{current_exe}\"\n"
            f"start \"\" \"{current_exe}\"\n"
            f"del \"{new_file}\"\n"
            f"del \"%~f0\"\n"
        )
        bat.close()
        subprocess.Popen(
            ["cmd", "/c", bat.name],
            creationflags=subprocess.CREATE_NO_WINDOW,
        )
    else:
        sh = tempfile.NamedTemporaryFile(delete=False, suffix=".sh", mode="w")
        sh.write(
            f"#!/bin/bash\n"
            f"sleep 1\n"
            f"cp -f \"{new_file}\" \"{current_exe}\"\n"
            f"chmod +x \"{current_exe}\"\n"
            f"\"{current_exe}\" &\n"
            f"rm -f \"{new_file}\" \"$0\"\n"
        )
        sh.close()
        os.chmod(sh.name, 0o755)
        subprocess.Popen(["/bin/bash", sh.name])

    QApplication.quit()

def _is_newer(latest: str, current: str) -> bool:
    def parse(v: str) -> tuple:
        try:
            return tuple(int(x) for x in v.split("."))
        except ValueError:
            return (0,)

    return parse(latest) > parse(current)


def _find_asset_url(assets: list) -> str | None:
    system = platform.system()
    machine = platform.machine().lower()
    is_arm = "arm" in machine or "aarch" in machine

    keywords = {
        "Windows": ["windows", "win"],
        "Darwin": ["macos", "mac", "darwin"],
        "Linux": ["linux"],
    }.get(system, [])

    candidates = [
        a for a in assets
        if any(kw in a["name"].lower() for kw in keywords)
    ]

    for asset in candidates:
        name = asset["name"].lower()
        if system == "Darwin":
            if is_arm and "arm64" in name:
                return asset["browser_download_url"]
            if not is_arm and "arm64" not in name:
                return asset["browser_download_url"]

    return candidates[0]["browser_download_url"] if candidates else None
