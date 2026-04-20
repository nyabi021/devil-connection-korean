import json
import re
import urllib.request
from dataclasses import dataclass


@dataclass(frozen=True)
class ReleaseInfo:
    version: str
    url: str
    name: str


def fetch_latest_release(api_url: str, timeout: int = 8) -> ReleaseInfo:
    request = urllib.request.Request(
        api_url,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": "devil-connection-korean-patcher",
        },
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        payload = json.loads(response.read().decode("utf-8"))

    version = str(payload.get("tag_name") or "").strip()
    url = str(payload.get("html_url") or "").strip()
    name = str(payload.get("name") or version).strip()
    if not version or not url:
        raise ValueError("릴리즈 정보를 읽을 수 없습니다.")

    return ReleaseInfo(version=version, url=url, name=name)


def is_newer_version(latest: str, current: str) -> bool:
    return _version_key(latest) > _version_key(current)


def _version_key(version: str) -> tuple[int, ...]:
    parts = re.findall(r"\d+", version)
    return tuple(int(part) for part in parts[:4]) or (0,)
