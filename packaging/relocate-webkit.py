"""Relocate distro WebKit paths without changing ELF offsets or its sandbox.

The release engine ignores WEBKIT_EXEC_PATH. Keep its matching helper processes
and injected bundle under the original relative directory and replace only
known /usr path prefixes by equal-length ././ prefixes. AppRun sets cwd=AppDir/usr.
"""
from pathlib import Path
import sys

appdir = Path(sys.argv[1]).resolve(strict=True)
original = sys.argv[2]
if not original.startswith('/usr/lib/') or '/webkitgtk-6.0' not in original:
    raise SystemExit('Unexpected WebKit installation path')
helper_dir = appdir / original.removeprefix('/')
if not helper_dir.is_dir():
    raise SystemExit('Matching WebKit helper directory is missing')
for name in ('WebKitWebProcess', 'WebKitNetworkProcess'):
    if not (helper_dir / name).is_file():
        raise SystemExit(f'Missing {name}')

prefixes = [original, '/usr/share/webkitgtk-6.0', '/usr/bin/bwrap', '/usr/bin/xdg-dbus-proxy']
libraries = list((appdir / 'usr/lib').glob('libwebkitgtk-6.0.so*'))
if not libraries:
    raise SystemExit('Bundled WebKit library missing')
count = 0
for library in libraries:
    if library.is_symlink():
        continue
    data = library.read_bytes()
    for prefix in prefixes:
        old = prefix.encode()
        new = ('././' + prefix[4:]).encode()
        assert len(old) == len(new)
        count += data.count(old)
        data = data.replace(old, new)
    library.write_bytes(data)
if count == 0:
    raise SystemExit('No known WebKit helper paths relocated; inspect the new distro package')
print(f'Relocated {count} known WebKit paths; sandbox remains enabled')
