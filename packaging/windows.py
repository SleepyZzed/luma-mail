#!/usr/bin/env python3
"""Build an auditable, relocatable x64 UCRT bundle, without a development toolchain.
Only the DLL dependency closure and selected local media plugins are distributed.
"""
import argparse, hashlib, json, os, re, shutil, subprocess, sys, zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent

def package(prefix,binary,output,version):
    prefix=prefix.resolve(strict=True);binary=binary.resolve(strict=True);output=output.resolve()
    if output.exists():raise SystemExit(f'Refusing to overwrite existing bundle: {output}')
    output.mkdir(parents=True)
    copied={};pending=[];libraries={p.name.lower():p for p in (prefix/'bin').glob('*') if p.is_file()}
    def copy(src,relative):
        target=output/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,target)
        copied[target.relative_to(output).as_posix()]=src
        if src.suffix.lower() in ('.exe','.dll'):pending.append(target)
        return target
    copy(binary,'luma-mail.exe')
    loader_dll=binary.parent/'WebView2Loader.dll'
    if not loader_dll.is_file():raise SystemExit('Run packaging/webview2-loader.py for this binary first')
    if hashlib.sha256(loader_dll.read_bytes()).hexdigest()!='8427b1fc58ec707813e5c0a51eb5d69397bb333250a7b891be4d3b123f1e0f1c':raise SystemExit('WebView2 SDK loader hash mismatch')
    copy(loader_dll,'WebView2Loader.dll')
    copy(ROOT/'packaging/WEBVIEW2-LICENSE.txt','licenses/WebView2-SDK-LICENSE.txt')
    # Runtime-loaded plugins are invisible to PE imports; include a deliberate set.
    plugins=('coreelements','playback','typefindfunctions','app','gio','audioconvert','audioresample','audioparsers','audiofx','videoconvertscale','videorate','volume','autodetect','directsound','wasapi','wasapi2','d3d11','d3d12','libav','vpx','isomp4','matroska','ogg','vorbis','opus','wavparse','flac','mpg123','avi','jpeg','png','adder','audiomixer')
    for plugin in plugins:
        src=prefix/f'lib/gstreamer-1.0/libgst{plugin}.dll'
        if src.exists():copy(src,src.relative_to(prefix))
    for required in ('coreelements','playback','typefindfunctions','audioconvert','audioresample','videoconvertscale'):
        if not (output/f'lib/gstreamer-1.0/libgst{required}.dll').is_file():raise SystemExit(f'Missing required media plugin: {required}')
    if not any((output/f'lib/gstreamer-1.0/libgst{p}.dll').is_file() for p in ('directsound','wasapi','wasapi2')):raise SystemExit('Missing Windows audio output plugin')
    scanner=prefix/'libexec/gstreamer-1.0/gst-plugin-scanner.exe';copy(scanner,scanner.relative_to(prefix))
    # Gtk4 and image previews need SVG decoding; JPEG/PNG are built into GdkPixbuf.
    loader_dir=prefix/'lib/gdk-pixbuf-2.0/2.10.0/loaders'
    loaders=[loader_dir/name for name in ('pixbufloader_svg.dll','libpixbufloader-png.dll','libpixbufloader-jpeg.dll','libpixbufloader-gif.dll','libpixbufloader-ico.dll','libpixbufloader-bmp.dll','libpixbufloader-tiff.dll')]
    for loader in loaders:
        if not loader.is_file():raise SystemExit(f'Missing image loader: {loader.name}')
        copy(loader,loader.relative_to(prefix))
    for lib in (prefix/'lib/gtk-4.0').rglob('*.dll') if (prefix/'lib/gtk-4.0').is_dir() else []:copy(lib,lib.relative_to(prefix))
    system=Path(os.environ['SystemRoot'])/'System32';checked=set()
    while pending:
        target=pending.pop()
        if target in checked:continue
        checked.add(target)
        headers=subprocess.check_output([str(prefix/'bin/objdump.exe'),'-p',str(target)],text=True,encoding='utf-8',errors='replace')
        for name in re.findall(r'DLL Name:\s*(\S+)',headers):
            if (output/name).is_file():continue
            src=libraries.get(name.lower())
            if src:copy(src,name)
            elif not (system/name).is_file() and not name.lower().startswith(('api-ms-win-','ext-ms-win-')):raise SystemExit(f'Unresolved dependency {name} needed by {target.name}')
    def tree(relative,filter=None):
        base=prefix/relative
        if base.is_dir():
            for src in base.rglob('*'):
                if src.is_file() and (filter is None or filter(src)):copy(src,src.relative_to(prefix))
    tree('share/glib-2.0/schemas',lambda p:p.suffix=='.xml')
    subprocess.run([str(prefix/'bin/glib-compile-schemas.exe'),str(output/'share/glib-2.0/schemas')],check=True)
    tree('share/icons/Adwaita/symbolic');tree('share/icons/Adwaita/scalable')
    copy(prefix/'share/icons/Adwaita/index.theme','share/icons/Adwaita/index.theme')
    tree('share/icons/hicolor',lambda p:p.name=='index.theme')
    tree('share/mime',lambda p:p.suffix=='.cache' or p.name in ('aliases','globs','globs2','magic','types','subclasses','treemagic','XMLnamespaces'))
    (output/'lib/gio/modules').mkdir(parents=True,exist_ok=True)
    if loaders:
        cache=subprocess.check_output([str(prefix/'bin/gdk-pixbuf-query-loaders.exe'),*[str(output/l.relative_to(prefix)) for l in loaders]],text=True,encoding='utf-8')
        cache=cache.replace(str(output).replace(chr(92),chr(92)*2),'@LUMA_BUNDLE@').replace(output.as_posix(),'@LUMA_BUNDLE@')
        (output/'lib/gdk-pixbuf-2.0/2.10.0/loaders.cache.in').write_text(cache,encoding='utf-8')
    # Pango uses installed system fonts. No Microsoft font binaries are redistributed.
    fonts=output/'etc/fonts/fonts.conf';fonts.parent.mkdir(parents=True,exist_ok=True)
    fonts.write_text('<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><dir>WINDOWSFONTDIR</dir><dir>WINDOWSUSERFONTDIR</dir><cachedir prefix="xdg">fontconfig</cachedir><config><rescan><int>0</int></rescan></config></fontconfig>',encoding='utf-8')
    copy(ROOT/'LICENSE','LICENSE.txt');copy(ROOT/'README.md','README.md');copy(ROOT/'data/luma.ico','luma.ico')
    copy(ROOT/'packaging/WINDOWS-README.txt','START-HERE.txt')
    # Record exact owning packages and their source-package links for DLL replacement
    # and licence/source inspection. Never ship MSYS2, Cargo or development tools.
    owners={};package_files={}
    local=prefix.parent/'var/lib/pacman/local'
    if not local.is_dir():raise SystemExit('MSYS2 package database is required for the dependency/source inventory')
    for entry in local.iterdir():
        desc=entry/'desc';files=entry/'files'
        if not desc.is_file() or not files.is_file():continue
        sections={m.group(1):m.group(2).strip().splitlines() for m in re.finditer(r'%([^%]+)%\n([^%]*)',desc.read_text(encoding='utf-8'))}
        name=sections.get('NAME',[''])[0];version_pkg=sections.get('VERSION',[''])[0];base=sections.get('BASE',[name])[0]
        file_list=files.read_text(encoding='utf-8').splitlines();package_files[name]=(entry,file_list)
        metadata={'name':name,'version':version_pkg,'base':base,'license':sections.get('LICENSE',[]),'upstream':sections.get('URL',[]),'source_package':f'https://repo.msys2.org/mingw/sources/{base}-{version_pkg}.src.tar.zst'}
        for file in file_list:
            if not file.endswith('/'):owners[file]=metadata
    used={};inventory=[]
    for relative,src in list(copied.items()):
        if src.is_relative_to(prefix):
            metadata=owners.get(src.relative_to(prefix.parent).as_posix())
            if not metadata and '/share/mime/' in src.as_posix():metadata=owners.get('ucrt64/share/mime/packages/freedesktop.org.xml')
            if not metadata:raise SystemExit(f'No MSYS2 ownership record: {src}')
            used[metadata['name']]=metadata;inventory.append({'file':relative,'package':metadata['name'],'sha256':hashlib.sha256((output/relative).read_bytes()).hexdigest()})
    for name,metadata in used.items():
        entry,files=package_files[name]
        for file in files:
            src=prefix.parent/file
            if '/share/licenses/' in file and src.is_file():copy(src,Path('licenses')/name/Path(file).name)
        copy(entry/'desc',Path('licenses')/name/'MSYS2-package.txt')
    (output/'THIRD-PARTY.json').write_text(json.dumps({'packages':list(used.values()),'files':inventory,'webview2_sdk':{'version':'1.0.3650.58','url':'https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3650.58','sha256':'911a472128c82ac8baa0c486c23342cc9dd6e7dc50d754e676726642ca065c60'}},indent=2)+'\n',encoding='utf-8')
    # Include our complete source as a separate release artifact; the tag pins it.
    archive=output.parent/f'Luma-Mail-{version}-windows-x86_64.zip'
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
        for p in sorted(output.rglob('*')):
            if p.is_file():z.write(p,Path(output.name)/p.relative_to(output))
    digest=hashlib.sha256(archive.read_bytes()).hexdigest();archive.with_suffix('.zip.sha256').write_text(f'{digest}  {archive.name}\n',encoding='ascii')
    report={'version':version,'archive_bytes':archive.stat().st_size,'installed_bytes':sum(p.stat().st_size for p in output.rglob('*') if p.is_file()),'dlls':sum(p.suffix.lower()=='.dll' for p in output.rglob('*')),'packages':len(used),'sha256':digest}
    archive.with_suffix('.storage.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8');print(json.dumps(report))
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--prefix',required=True,type=Path);p.add_argument('--binary',required=True,type=Path);p.add_argument('--output',required=True,type=Path);p.add_argument('--version',required=True);args=p.parse_args();package(args.prefix,args.binary,args.output,args.version)
