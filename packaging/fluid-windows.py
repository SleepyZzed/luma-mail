#!/usr/bin/env python3
"""Create a GTK-free Qt Quick portable preview; never includes a mailbox or SDK."""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile, zipfile
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
LOADER_SHA = '8427b1fc58ec707813e5c0a51eb5d69397bb333250a7b891be4d3b123f1e0f1c'
def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()
def package(args):
    prefix=args.prefix.resolve(strict=True); output=args.output.resolve()
    if output.exists(): raise SystemExit(f'Refusing to overwrite an existing package: {output}')
    if not re.fullmatch(r'[0-9A-Za-z][0-9A-Za-z.+_-]{0,100}',args.version): raise SystemExit('Invalid package version')
    deploy=prefix/'bin/windeployqt.exe'; objdump=prefix/'bin/objdump.exe'
    for tool in (deploy,objdump):
        if not tool.is_file(): raise SystemExit(f'Required packaging tool is missing: {tool.name}')
    source_files={}; output.mkdir(parents=True); binary_dir=output/'bin';binary_dir.mkdir()
    def copy(source, relative):
        source=source.resolve(strict=True);target=output/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target);source_files[target.relative_to(output).as_posix()]=source
    copy(args.binary,'bin/luma-mail.exe');copy(args.engine,'bin/luma-engine.exe');copy(args.webview,'bin/luma_webview.dll')
    loader=args.webview.resolve().parent/'WebView2Loader.dll'
    if not loader.is_file() or digest(loader)!=LOADER_SHA: raise SystemExit('Stage the pinned WebView2 loader with packaging/webview2-loader.py first')
    copy(loader,'bin/WebView2Loader.dll')
    # The UI is compiled into the executable. Scan only the Windows imports,
    # excluding the Linux-only WebEngine adapter from the temporary source view.
    with tempfile.TemporaryDirectory(prefix='.luma-qml-',dir=output.parent) as scratch:
        for source in (ROOT/'crates/fluid/qml').glob('*.qml'):
            if source.name!='LinuxReader.qml':shutil.copy2(source,Path(scratch)/source.name)
        subprocess.run([str(deploy),'--release','--no-translations','--qmldir',scratch,
                        '--dir',str(output),'--libdir',str(binary_dir),
                        '--plugindir',str(output/'plugins'),'--qml-deploy-dir',str(output/'qml'),
                        str(binary_dir/'luma-mail.exe')],check=True)
    (binary_dir/'qt.conf').write_text('[Paths]\nPrefix=..\nBinaries=bin\nLibraries=bin\nPlugins=plugins\nQmlImports=qml\n',encoding='utf8')
    libraries={p.name.lower():p for p in (prefix/'bin').glob('*.dll')}
    system=Path(os.environ['SystemRoot'])/'System32';pending=list(output.rglob('*.exe'))+list(output.rglob('*.dll'));checked=set()
    while pending:
        target=pending.pop()
        if target in checked:continue
        checked.add(target)
        headers=subprocess.check_output([str(objdump),'-p',str(target)],text=True,encoding='utf8',errors='replace')
        for name in re.findall(r'DLL Name:\s*(\S+)',headers):
            if (binary_dir/name).is_file() or (target.parent/name).is_file():continue
            source=libraries.get(name.lower())
            if source:copy(source,Path('bin')/name);pending.append(binary_dir/name)
            elif not (system/name).is_file() and not name.lower().startswith(('api-ms-win-','ext-ms-win-')):raise SystemExit(f'Unresolved dependency {name} required by {target.name}')
    for file in output.rglob('*'):
        if file.is_file() and re.search(r'(libgtk[-.]|libadwaita|webkit|libgdk-[34]|gstreamer)',file.name,re.I):raise SystemExit(f'Unexpected legacy UI runtime: {file.name}')
    # Match deployed Qt plugins back to MSYS2's exact owning package. Keep source
    # links and notices so the bundled shared libraries can be inspected/replaced.
    local=prefix.parent/'var/lib/pacman/local';owners={};packages={}
    if not local.is_dir():raise SystemExit('The MSYS2 ownership database is required')
    for entry in local.iterdir():
        desc=entry/'desc';files=entry/'files'
        if not desc.is_file() or not files.is_file():continue
        sections={m.group(1):m.group(2).strip().splitlines() for m in re.finditer(r'%([^%]+)%\n([^%]*)',desc.read_text(encoding='utf8'))}
        name=sections.get('NAME',[''])[0];version=sections.get('VERSION',[''])[0];base=sections.get('BASE',[name])[0]
        metadata={'name':name,'version':version,'license':sections.get('LICENSE',[]),'upstream':sections.get('URL',[]),'source_package':f'https://repo.msys2.org/mingw/sources/{base}-{version}.src.tar.zst'}
        paths=[prefix.parent/f for f in files.read_text(encoding='utf8').splitlines() if not f.startswith('%') and f and not f.endswith('/')]
        packages[name]=(entry,metadata,paths)
        for source in paths:owners[source]=name
    candidates={}
    for source in owners:
        if source.suffix.lower() in ('.dll','.exe','.qml','.qmltypes') or source.name=='qmldir':candidates.setdefault(source.name.lower(),[]).append(source)
    inventory=[];used=set()
    for target in list(output.rglob('*')):
        if not target.is_file():continue
        relative=target.relative_to(output).as_posix();source=source_files.get(relative)
        if source is None:
            choices=candidates.get(target.name.lower(),[])
            if choices:
                sha=digest(target);source=next((p for p in choices if p.is_file() and digest(p)==sha),None)
                # windeployqt may rewrite QtCore's installation prefix. Its
                # owning binary is still unambiguous by the bin-relative path.
                if source is None and relative.startswith('bin/') and (prefix/'bin'/target.name) in owners:source=prefix/'bin'/target.name
        owner=owners.get(source)
        if source is not None and source.is_relative_to(prefix) and not owner:raise SystemExit(f'Missing package ownership for {relative}')
        if source is None and target.suffix.lower() in ('.dll','.exe'):raise SystemExit(f'Unattributed deployed binary: {relative}')
        if owner:used.add(owner)
        inventory.append({'file':relative,'sha256':digest(target),'package':owner or 'Luma/source-or-generated'})
    for owner in sorted(used):
        entry,_,paths=packages[owner]
        for source in paths:
            if '/share/licenses/' in source.as_posix() and source.is_file():copy(source,Path('licenses')/owner/source.name)
        copy(entry/'desc',Path('licenses')/owner/'MSYS2-package.txt')
    for source,relative in [('LICENSE','LICENSE.txt'),('docs/fluid-packaging.md','START-HERE.md'),('packaging/WEBVIEW2-LICENSE.txt','licenses/WEBVIEW2-LICENSE.txt'),('Cargo.lock','source/Cargo.lock')]:copy(ROOT/source,relative)
    subprocess.run([os.sys.executable,str(ROOT/'packaging/rust-notices.py'),'--windows','--output',str(output/'licenses/rust')],check=True,cwd=ROOT)
    changes=subprocess.check_output(['git','status','--porcelain','--untracked-files=all','--','crates','packaging','data','Cargo.toml','Cargo.lock','.github','docs'],cwd=ROOT,text=True).splitlines()
    if changes:print('Source provenance changes: '+json.dumps(changes))
    metadata={'schema':1,'frontend':'Qt Quick (GTK-free)','version':args.version,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_dirty':bool(changes),'source_changes':changes,'packages':[packages[name][1] for name in sorted(used)],'files':inventory,'webview2_sdk':{'version':'1.0.3650.58','sha256':LOADER_SHA,'source':'https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3650.58'}}
    (output/'THIRD-PARTY.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf8')
    archive=output.parent/f'Luma-Mail-{args.version}-fluid-windows-x86_64.zip'
    if archive.exists():raise SystemExit(f'Refusing to overwrite {archive}')
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as bundle:
        for file in sorted(output.rglob('*')):
            if file.is_file():bundle.write(file,Path(output.name)/file.relative_to(output))
    sha=digest(archive);archive.with_suffix('.zip.sha256').write_text(f'{sha}  {archive.name}\n',encoding='ascii')
    report={'version':args.version,'source_commit':metadata['source_commit'],'source_dirty':metadata['source_dirty'],'archive_bytes':archive.stat().st_size,'installed_logical_bytes':sum(p.stat().st_size for p in output.rglob('*') if p.is_file()),'files':sum(p.is_file() for p in output.rglob('*')),'packages':len(used),'sha256':sha,'mailbox_bytes':0,'note':'Package bytes only. Runtime/settings/cache allocation is measured separately; no account data is copied.'}
    archive.with_suffix('.storage.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8');print(json.dumps(report))
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('prefix','binary','engine','webview','output'):parser.add_argument('--'+name,required=True,type=Path)
    parser.add_argument('--version',required=True);package(parser.parse_args())
