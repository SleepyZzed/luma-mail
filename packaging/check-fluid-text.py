#!/usr/bin/env python3
"""Reject accidental Windows-codepage conversions of the English UI punctuation."""
from pathlib import Path
import sys

root = Path(__file__).resolve().parent.parent
paths = sorted((root / 'crates/fluid/qml').glob('*.qml'))
paths += sorted((root / 'docs').glob('*.md'))
paths += [root / 'README.md', root / 'STATUS.md']
bad = tuple(c.encode('utf-8').decode('cp1252') for c in '\u00b7\u2026\u2019\u2013\u2014\u2192')
errors = []
for path in paths:
    try:
        text = path.read_text(encoding='utf-8-sig')
    except UnicodeDecodeError:
        errors.append(f'{path.relative_to(root)}: not valid UTF-8')
        continue
    for line, value in enumerate(text.splitlines(), 1):
        if any(pattern in value for pattern in bad):
            errors.append(f'{path.relative_to(root)}:{line}: converted punctuation; use explicit UTF-8 text I/O')
if errors:
    print('\n'.join(errors), file=sys.stderr)
    raise SystemExit(1)
print(f'Checked UTF-8 UI punctuation in {len(paths)} files')
