#!/usr/bin/env python3
"""Retain licence/source records only for the private engine and Windows adapter."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--windows', action='store_true')
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
compiler = subprocess.check_output(['rustc', '-Vv'], text=True)
target = re.search(r'^host: (.+)$', compiler, re.M).group(1)
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version', '1', '--filter-platform', target], cwd=ROOT))
packages = {p['id']: p for p in metadata['packages']}
nodes = {n['id']: n for n in metadata['resolve']['nodes']}
roots = ['luma-engine', 'luma-webview'] if args.windows else ['luma-engine']
pending = [p['id'] for p in packages.values() if p['name'] in roots]
seen = set()
records = []
while pending:
    identity = pending.pop()
    if identity in seen: continue
    seen.add(identity)
    pending.extend(n['pkg'] for n in nodes[identity]['deps'])
    package = packages[identity]
    if package['source'] is None: continue
    base = Path(package['manifest_path']).parent.resolve()
    dest = output / f"{package['name']}-{package['version']}"
    candidates = [p for p in base.iterdir() if p.is_file() and re.match(r'(?i)^(license|copying|copyright|notice)(\b|[._-])', p.name)]
    if package.get('license_file'): candidates.append(base / package['license_file'])
    files = []
    for source in dict.fromkeys(candidates):
        if not source.resolve().is_relative_to(base): raise ValueError('Licence path leaves its source package')
        if source.stat().st_size > 2 * 1024 * 1024: raise ValueError('Oversized source notice')
        dest.mkdir(parents=True, exist_ok=True)
        target_file = dest / source.name
        shutil.copyfile(source, target_file)
        files.append(dict(file=target_file.relative_to(output).as_posix(), sha256=hashlib.sha256(target_file.read_bytes()).hexdigest()))
    records.append(dict(name=package['name'], version=package['version'], license=package['license'],
                        source=package['source'], repository=package['repository'], authors=package['authors'],
                        corresponding_source=f"https://crates.io/api/v1/crates/{package['name']}/{package['version']}/download"
                        if package['source'].startswith('registry+') else package['source'], notices=files))
# The statically linked standard library is supplied by the exact compiler build.
# Keep its notices as well as the version/source commit; no compiler is bundled.
commit = re.search(r'^commit-hash: ([0-9a-f]{40})$', compiler, re.M).group(1)
standard = output / 'Rust-standard-library'
standard.mkdir(exist_ok=True)
for name in ['COPYRIGHT', 'LICENSE-MIT', 'LICENSE-APACHE']:
    with urlopen(f'https://raw.githubusercontent.com/rust-lang/rust/{commit}/{name}', timeout=60) as response:
        data = response.read(2 * 1024 * 1024 + 1)
        if len(data) > 2 * 1024 * 1024: raise ValueError('Oversized standard-library notice')
        (standard / name).write_bytes(data)
manifest = dict(schema=1, roots=roots, target=target, compiler=compiler,
                standard_library_source=f'https://github.com/rust-lang/rust/tree/{commit}',
                packages=sorted(records, key=lambda p: (p['name'], p['version'])))
(output / 'RUST-SOURCES.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
print(json.dumps(dict(rust_packages=len(records), target=target, standard_library_commit=commit)))
