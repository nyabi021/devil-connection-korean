import sys

from PyQt6.QtGui import QFont
from PyQt6.QtWidgets import QApplication

from ui import KoreanPatchInstaller
from utils import get_system_font


def main():
    app = QApplication(sys.argv)
    app.setFont(QFont(get_system_font(), 10))

    window = KoreanPatchInstaller()
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
