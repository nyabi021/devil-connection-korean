import json
import sys
from pathlib import Path

def _base_path() -> Path:
    return Path(getattr(sys, '_MEIPASS', Path(__file__).parent))

def load() -> dict:
    with open(_base_path() / 'config.json', encoding='utf-8') as f:
        return json.load(f)

_cfg = load()

VERSION: str = _cfg['version']
GITHUB_OWNER: str = _cfg['github']['owner']
GITHUB_REPO: str = _cfg['github']['repo']
GITHUB_API_LATEST: str = (
    f"https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
)
GITHUB_RELEASES_URL: str = (
    f"https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
)

APP_TITLE: str = _cfg['app']['title']
WINDOW_WIDTH: int = _cfg['app']['window_width']
WINDOW_HEIGHT: int = _cfg['app']['window_height']
CREDITS: str = _cfg['app']['credits']

PATCH_DIRS: list[str] = _cfg['patch']['dirs']

BASE_PATH: Path = _base_path()
