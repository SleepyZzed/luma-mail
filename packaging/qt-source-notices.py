#!/usr/bin/env python3
"""Retain matching Qt/Chromium source notices without installing source archives."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import time
from urllib.request import urlopen

MODULES = ['qtbase', 'qtdeclarative', 'qtmultimedia', 'qtsvg', 'qtwebengine',
           'qtwebchannel', 'qtpositioning', 'qtwayland']
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--version', required=True)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--review-unpinned', action='store_true', help='Maintainer source-review collection before adding an exact source pin')
args = parser.parse_args()
if not re.fullmatch(r'6\.[0-9]+\.[0-9]+', args.version): raise SystemExit('An exact Qt 6 source version is required')
pins=json.loads(Path(__file__).with_name('qt-source-pins.json').read_text(encoding='utf-8'))
pin=pins.get(args.version)
if pin is None and not args.review_unpinned:raise SystemExit('This Qt source version needs a reviewed source pin first')
def fingerprint(data):return hashlib.sha256(json.dumps(data,sort_keys=True,separators=(',',':')).encode()).hexdigest()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
manifest = output / 'QT-SOURCES.json'
if manifest.exists():
    cached = json.loads(manifest.read_text(encoding='utf-8'))
    if (pin is None or fingerprint(cached)==pin['manifest_sha256']) and cached.get('version') == args.version and sorted(m['module'] for m in cached['modules']) == sorted(MODULES) and all(m['notices'] for m in cached['modules']) and all(
        (output / item['file']).is_file() and hashlib.sha256((output / item['file']).read_bytes()).hexdigest() == item['sha256']
        for module in cached['modules'] for item in module['notices']
    ):
        print('Verified cached Qt source notices for ' + args.version)
        raise SystemExit(0)
total = 0
modules = []
for module in MODULES:
    url = f'https://download.qt.io/archive/qt/{args.version.rsplit(".", 1)[0]}/{args.version}/submodules/{module}-everywhere-src-{args.version}.tar.xz'
    for attempt in range(3):
        try:
            with urlopen(url, timeout=60) as response:
                class Stream:
                    sha = None
                    size = 0
                    def __init__(self): self.sha = hashlib.sha256()
                    def read(self, n=-1):
                        data = response.read(n)
                        self.size += len(data)
                        if self.size > 768 * 1024 * 1024: raise ValueError('Source archive exceeds the reviewed bound')
                        self.sha.update(data)
                        return data
                stream = Stream()
                notices = []
                module_bytes = 0
                with tarfile.open(fileobj=stream, mode='r|xz') as archive:
                    for member in archive:
                        if not member.isfile(): continue
                        path = PurePosixPath(member.name)
                        if path.is_absolute() or '..' in path.parts or len(path.parts) < 2: raise ValueError('Unsafe source archive path')
                        relative = PurePosixPath(*path.parts[1:])
                        selected = (re.match(r'(?i)^(license|copying|copyright|notice|patents|authors)(\b|[._-])', relative.name)
                                    or relative.name in ['qt_attribution.json', 'README.chromium']
                                    or 'LICENSES' in relative.parts or str(relative).endswith('.reuse/dep5'))
                        if not selected: continue
                        if member.size > 2 * 1024 * 1024 or module_bytes + member.size > 64 * 1024 * 1024: raise ValueError('Source notices exceed the reviewed bound')
                        data = archive.extractfile(member).read(2 * 1024 * 1024 + 1)
                        target = output / module / Path(str(relative))
                        target.parent.mkdir(parents=True, exist_ok=True)
                        target.write_bytes(data)
                        module_bytes += len(data)
                        notices.append(dict(file=target.relative_to(output).as_posix(), sha256=hashlib.sha256(data).hexdigest()))
                while stream.read(1024 * 1024): pass
                expected = response.headers.get('Content-Length')
                if expected and stream.size != int(expected): raise ValueError('Incomplete Qt source archive')
                if not notices: raise ValueError('No source notices found')
                if pin and stream.sha.hexdigest()!=pin['archives'][module]:raise ValueError('Qt source archive differs from its reviewed pin')
                record = dict(module=module, version=args.version, source=url, archive_bytes=stream.size,
                              source_sha256=stream.sha.hexdigest(), notice_bytes=module_bytes, notices=notices)
            modules.append(record)
            total += module_bytes
            print(json.dumps({key: value for key, value in record.items() if key != 'notices'}), flush=True)
            break
        except (OSError, ValueError, tarfile.TarError):
            if attempt == 2: raise
            time.sleep(2 ** attempt)
result=dict(version=args.version,modules=modules,notice_bytes=total)
if pin and fingerprint(result)!=pin['manifest_sha256']:raise ValueError('Collected notices differ from the reviewed source inventory')
manifest.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
