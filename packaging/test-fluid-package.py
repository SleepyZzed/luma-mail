#!/usr/bin/env python3
"""Exercise an exposed portable build with synthetic mail and no SDK search paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

CASES = ['auth', 'popout', 'popup', 'navigation', 'composer', 'motion', 'photo', 'html',
         'thread', 'search', 'conflict', 'sync', 'attachment', 'signature', 'recovery']
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--cases', nargs='+', choices=CASES, default=CASES)
args = parser.parse_args()
binary = args.binary.resolve(strict=True)
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=True)
checks = []
for case in args.cases:
    env = dict(os.environ)
    for name in ['LUMA_ENGINE', 'LUMA_WEBVIEW', 'LUMA_TEST_ROOT', 'QT_PLUGIN_PATH',
                 'QML_IMPORT_PATH', 'QML2_IMPORT_PATH', 'QT_QPA_PLATFORM_PLUGIN_PATH',
                 'LD_LIBRARY_PATH', 'QTDIR', 'QT_ROOT_DIR', 'LUMA_PREVIEW_AUTH',
                 'LUMA_PREVIEW_HTML', 'LUMA_PREVIEW_PHOTOS', 'LUMA_PREVIEW_THREAD_COUNT',
                 'LUMA_PREVIEW_CONFLICT', 'LUMA_PREVIEW_READER_FAILURES']:
        env.pop(name, None)
    env['PATH'] = (os.environ['SystemRoot'] + '\\System32;' + os.environ['SystemRoot']) if sys.platform == 'win32' else '/usr/bin:/bin'
    env.update(QT_FORCE_STDERR_LOGGING='1', LUMA_PREVIEW_TWO_ACCOUNTS='1', LUMA_PREVIEW_THEME='dark')
    if case == 'auth': env['LUMA_PREVIEW_AUTH'] = '1'
    if case == 'photo': env['LUMA_PREVIEW_PHOTOS'] = '1'
    if case in ['html', 'recovery']: env['LUMA_PREVIEW_HTML'] = '1'
    if case == 'thread': env['LUMA_PREVIEW_THREAD_COUNT'] = '2503'
    if case == 'conflict': env['LUMA_PREVIEW_CONFLICT'] = '1'
    if case == 'recovery': env['LUMA_PREVIEW_READER_FAILURES'] = '1'
    report = output / f'{case}.json'
    report.unlink(missing_ok=True)
    env['LUMA_' + ('PROFILE' if case == 'photo' else case.upper()) + '_REPORT'] = str(report)
    # AppImage extraction is private to its runtime and removed on exit. Never
    # disable Chromium's sandbox or change the user's security configuration.
    if sys.platform != 'win32': env['APPIMAGE_EXTRACT_AND_RUN'] = '1'
    try:
        process = subprocess.run([str(binary), '--design-preview', f'--{case}-test'],
                                 cwd=output, env=env, stdin=subprocess.DEVNULL,
                                 capture_output=True, timeout=45)
        (output / f'{case}.stdout').write_bytes(process.stdout)
        (output / f'{case}.stderr').write_bytes(process.stderr)
        data = json.loads(report.read_text(encoding='utf-8')) if report.exists() else {}
        warnings = [line for line in process.stderr.decode(errors='replace').splitlines()
                    if any(word in line for word in ['Binding loop', 'TypeError', 'ReferenceError', 'Cannot open'])]
        item = dict(case=case, passed=process.returncode == 0 and data.get('passed') is True and not warnings,
                    exit_code=process.returncode, checks=len(data.get('checks', [])), qml_warnings=warnings)
    except (subprocess.TimeoutExpired, ValueError, OSError) as error:
        item = dict(case=case, passed=False, error=str(error))
    checks.append(item)
    print(json.dumps(item), flush=True)
    if not item['passed']: break
with binary.open('rb') as stream:
    sha = hashlib.file_digest(stream, 'sha256').hexdigest()
summary = dict(passed=all(item['passed'] for item in checks), synthetic_data=True,
               binary_sha256=sha, platform=sys.platform, sdk_search_paths_removed=True,
               checks=checks, total_checks=sum(item.get('checks', 0) for item in checks))
(output / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n', encoding='utf-8')
raise SystemExit(0 if summary['passed'] else 2)
