import json
import sys
from pathlib import Path


def _base_path() -> Path:
    meipass = getattr(sys, "_MEIPASS", None)
    if meipass is not None:
        return Path(meipass)
    return Path(__file__).parent


def load() -> dict:
    with open(_base_path() / "config.json", encoding="utf-8") as f:
        return json.load(f)


_cfg = load()

APP_TITLE: str = _cfg["app"]["title"]
APP_VERSION: str = _cfg["app"]["version"]
WINDOW_WIDTH: int = _cfg["app"]["window_width"]
WINDOW_HEIGHT: int = _cfg["app"]["window_height"]
CREDITS: str = _cfg["app"]["credits"]
RELEASE_API_URL: str = _cfg["app"]["release_api_url"]
RELEASES_URL: str = _cfg["app"]["releases_url"]

PATCH_DIRS: list[str] = _cfg["patch"]["dirs"]

BASE_PATH: Path = _base_path()
