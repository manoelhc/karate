"""Install a checksum-pinned, workspace-local Node; no global system changes."""
import hashlib
from pathlib import Path
import platform
import shutil
import tarfile
import urllib.request

VERSION = '26.13.0'
HASHES = {
    'darwin-arm64': '6f5b3187d9df9fc007e881167905bc7d69ad26450ddc377c83d63256a2495b6e',
    'darwin-x64': '8bd636bc981a92e1ab930c02340eece2076eb1aaf94d14e421cdeeddc1292298',
    'linux-arm64': '406fe3fbde1c054adff8b297c16fdb5af6659db327c1ac8acd46b56ef1af9a92',
    'linux-x64': 'd28a96af6c38596f6d9d97c0ee531c13d238d757742387d1928de822d959e997',
}
arch = {'arm64':'arm64','aarch64':'arm64','x86_64':'x64'}[platform.machine()]
system = f'{platform.system().lower()}-{arch}'
root = Path(__file__).resolve().parents[2]/'.local/node-download'
root.mkdir(parents=True, exist_ok=True)
name = f'node-v{VERSION}-{system}'
if not (root/name/'bin/node').exists():
    archive = root/f'{name}.tar.xz'
    try:
        with urllib.request.urlopen(f'https://nodejs.org/dist/v{VERSION}/{archive.name}', timeout=60) as response, archive.open('wb') as output:
            shutil.copyfileobj(response, output)
        with archive.open('rb') as downloaded:
            if hashlib.file_digest(downloaded, 'sha256').hexdigest() != HASHES[system]:
                raise ValueError('Node checksum mismatch')
        with tarfile.open(archive) as package:
            package.extractall(root, filter='data')
        # This application uses published binaries, never locally built Node addons.
        shutil.rmtree(root/name/'include')
    finally:
        archive.unlink(missing_ok=True)
print(f'Workspace Node {VERSION} ready ({system}).')
