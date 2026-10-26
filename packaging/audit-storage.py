#!/usr/bin/env python3
"""Read-only whole-app footprint report; never follows links or reads file contents."""
import argparse
import json
import os
import stat
from pathlib import Path


def measure(path):
    result = dict(logical_bytes=0, allocated_bytes=0, files=0, directories=0, links=0)
    seen = set()
    todo = [Path(path)]
    while todo:
        current = todo.pop()
        try:
            info = current.lstat()
        except FileNotFoundError:
            continue
        identity = (info.st_dev, info.st_ino)
        if identity in seen:
            continue
        seen.add(identity)
        result["allocated_bytes"] += getattr(info, "st_blocks", (info.st_size + 511) // 512) * 512
        if stat.S_ISLNK(info.st_mode):
            result["links"] += 1
        elif stat.S_ISDIR(info.st_mode):
            result["directories"] += 1
            with os.scandir(current) as entries:
                todo.extend(Path(entry.path) for entry in entries)
        elif stat.S_ISREG(info.st_mode):
            result["files"] += 1
            result["logical_bytes"] += info.st_size
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tree", nargs=2, action="append", default=[], metavar=("LABEL", "PATH"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = {"schema": 1, "allocation_method": "st_blocks on Unix; 512-byte estimate elsewhere",
              "categories": {label: measure(path) for label, path in args.tree}}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")
