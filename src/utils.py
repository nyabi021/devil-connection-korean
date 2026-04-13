import platform

def get_system_font() -> str:
    system = platform.system()
    if system == "Darwin":
        return "Apple SD Gothic Neo"
    elif system == "Windows":
        return "Malgun Gothic"
    return "Noto Sans CJK KR"

def get_monospace_font() -> str:
    system = platform.system()
    if system == "Darwin":
        return "Menlo"
    elif system == "Windows":
        return "Consolas"
    return "DejaVu Sans Mono"
