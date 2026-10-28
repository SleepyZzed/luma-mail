#!/usr/bin/env python3
"""Read-only whole-app footprint report; never follows links or reads file contents."""
import argparse
import json
import os
import stat
from pathlib import Path

if os.name == 'nt':
    import ctypes
    from ctypes import wintypes

    class StandardInfo(ctypes.Structure):
        _fields_ = [('allocation', ctypes.c_longlong), ('end', ctypes.c_longlong),
                    ('links', wintypes.DWORD), ('pending', ctypes.c_ubyte),
                    ('directory', ctypes.c_ubyte)]

    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                  ctypes.c_void_p, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    kernel.CreateFileW.restype = wintypes.HANDLE
    kernel.GetFileInformationByHandleEx.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD]
    kernel.GetFileInformationByHandleEx.restype = wintypes.BOOL
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL


def allocation(path, info):
    if os.name != 'nt':
        return getattr(info, 'st_blocks', (info.st_size + 511) // 512) * 512
    # Query metadata without reading contents, following reparse points or
    # blocking normal readers/writers. Include directory allocation as reported
    # by the filesystem; shared MFT/journal metadata is outside this tree audit.
    handle = kernel.CreateFileW(str(path.absolute()), 0, 7, None, 3, 0x02200000, None)
    if handle == wintypes.HANDLE(-1).value:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        data = StandardInfo()
        if not kernel.GetFileInformationByHandleEx(handle, 1, ctypes.byref(data), ctypes.sizeof(data)):
            raise ctypes.WinError(ctypes.get_last_error())
        return data.allocation
    finally:
        kernel.CloseHandle(handle)


def measure(path):
    result = dict(logical_bytes=0, allocated_bytes=0, files=0, directories=0, links=0)
    seen = set()
    groups = {}
    root = Path(path)
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
        allocated = allocation(current, info)
        if current != root:
            name = current.relative_to(root).parts[0]
            group = groups.setdefault(name, dict(logical_bytes=0, allocated_bytes=0, files=0))
            group["allocated_bytes"] += allocated
            if stat.S_ISREG(info.st_mode):
                group["logical_bytes"] += info.st_size
                group["files"] += 1
        result["allocated_bytes"] += allocated
        if stat.S_ISLNK(info.st_mode):
            result["links"] += 1
        elif stat.S_ISDIR(info.st_mode):
            result["directories"] += 1
            with os.scandir(current) as entries:
                todo.extend(Path(entry.path) for entry in entries)
        elif stat.S_ISREG(info.st_mode):
            result["files"] += 1
            result["logical_bytes"] += info.st_size
    result["top_level"] = dict(sorted(groups.items()))
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tree", nargs=2, action="append", default=[], metavar=("LABEL", "PATH"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = {"schema": 2, "allocation_method": "Win32 FileStandardInfo (no reparse traversal)" if os.name == 'nt' else "st_blocks on Unix; 512-byte estimate elsewhere",
              "categories": {label: measure(path) for label, path in args.tree}}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")
