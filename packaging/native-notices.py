"""Preserve distro notices and exact source-package links for bundled binaries."""
from pathlib import Path
import shutil
import subprocess
import sys
from urllib.parse import quote

def bundled_native_names(appdir):
    # Qt/Rust notices have exact-version bundles. Their generic copyright/README
    # filenames must never select unrelated packages from the build machine.
    root = Path(appdir).resolve(strict=True)
    names = set()
    for file in (root / 'usr').rglob('*'):
        if not file.is_file():
            continue
        if not file.resolve().is_relative_to(root):
            raise ValueError('Deployed file link leaves the application directory')
        with file.open('rb') as stream:
            if stream.read(4) == b'\x7fELF':
                names.add(file.name)
    if not names:
        raise ValueError('No deployed ELF binaries were found')
    return names


def inventory_owners(inventory, names):
    packages = set()
    for line in inventory.splitlines():
        owner, sep, path = line.partition(': ')
        if sep and not owner.startswith('diversion') and Path(path).name in names:
            packages.update(owner.split(', '))
    return packages


def main():
    appdir=Path(sys.argv[1]).resolve(strict=True)
    notices=appdir/'usr/share/licenses/luma-mail/native'
    notices.mkdir(parents=True,exist_ok=True)
    names=bundled_native_names(appdir)
    # Query the installed file inventory once. Reopening dpkg's database separately
    # for every bundled icon/library made packaging needlessly slow.
    inventory=subprocess.check_output(['dpkg-query','-S','*'],text=True)
    packages=inventory_owners(inventory,names)
    rows=['Binary package\tVersion\tSource package\tSource version\tCorresponding source']
    for package in sorted(packages):
        result=subprocess.check_output(['dpkg-query','-W','-f=${binary:Package}\t${Version}\t${source:Package}\t${source:Version}',package],text=True)
        binary,version,source,source_version=result.split('\t')
        source=source or binary.split(':')[0]; source_version=source_version or version
        url='https://launchpad.net/ubuntu/+source/'+quote(source)+'/'+quote(source_version,safe='')
        # Source-package page lists the original tarballs, distro patch set and .dsc.
        rows.append('\t'.join((binary,version,source,source_version,url)))
        copyright_file=Path('/usr/share/doc')/binary.split(':')[0]/'copyright'
        if copyright_file.is_file():
            target=notices/binary.replace(':','_');target.mkdir(exist_ok=True)
            shutil.copyfile(copyright_file,target/'copyright')
    for file in Path('/usr/share/common-licenses').iterdir():
        if file.is_file():
            target=notices/'common-licenses';target.mkdir(exist_ok=True)
            shutil.copyfile(file,target/file.name)
    manifest='\n'.join(rows)+'\n'
    (notices/'NATIVE-SOURCES.tsv').write_text(manifest,encoding='utf-8')
    destination=Path(sys.argv[2]) if len(sys.argv)>2 else Path('dist/NATIVE-SOURCES.tsv')
    destination.parent.mkdir(parents=True,exist_ok=True)
    destination.write_text(manifest,encoding='utf-8')
    print(f'Preserved {len(packages)} distro package notices and corresponding source links')


if __name__ == '__main__':
    main()
