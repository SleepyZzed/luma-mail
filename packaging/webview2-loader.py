#!/usr/bin/env python3
"""Stage the GNU dynamic loader, pinned to the exact official SDK DLL in Cargo.lock."""
import argparse,hashlib,shutil
from pathlib import Path
SDK='1.0.3650.58'
DLL_SHA256='8427b1fc58ec707813e5c0a51eb5d69397bb333250a7b891be4d3b123f1e0f1c'
SDK_SHA256='911a472128c82ac8baa0c486c23342cc9dd6e7dc50d754e676726642ca065c60'
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--binary',required=True,type=Path);args=p.parse_args()
    binary=args.binary.resolve(strict=True)
    candidates=list((binary.parent/'build').glob('webview2-com-sys-*/out/x64/WebView2Loader.dll'))
    source=next((f for f in candidates if hashlib.sha256(f.read_bytes()).hexdigest()==DLL_SHA256),None)
    if source is None:raise SystemExit('The expected WebView2 SDK loader is missing; update and review the SDK/license pin before distribution')
    shutil.copy2(source,binary.parent/'WebView2Loader.dll')
    print(f'WebView2 SDK {SDK} x64 loader verified and staged')
