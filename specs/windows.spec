import os
block_cipher = None

root = os.path.abspath(os.path.join(SPECPATH, '..'))
src = os.path.join(root, 'src')

datas = [
    (os.path.join(src, 'config.json'), '.'),
    (os.path.join(root, 'data/scenario'), 'data/scenario'),
    (os.path.join(root, 'data/others'), 'data/others'),
    (os.path.join(root, 'data/system'), 'data/system'),
    (os.path.join(root, 'data/fgimage'), 'data/fgimage'),
    (os.path.join(root, 'data/image'), 'data/image'),
    (os.path.join(root, 'data/video'), 'data/video'),
    (os.path.join(root, 'data/bgimage'), 'data/bgimage'),
    (os.path.join(root, 'tyrano'), 'tyrano'),
]

a = Analysis(
    [os.path.join(src, 'main.py')],
    pathex=[src],
    binaries=[],
    datas=datas,
    hiddenimports=[
        'PyQt6.QtCore',
        'PyQt6.QtGui',
        'PyQt6.QtWidgets',
        'asar',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='DevilConnection-Patcher-Windows-x64',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,  
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon='icons/icon.ico',  
)
