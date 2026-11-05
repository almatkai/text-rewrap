#!/usr/bin/env python3
"""Verify a package ZIP and exercise its extracted executable offline.

Never installs it. A recording stub host answers typed reverse RPCs with canned
values (no OS clipboard/selection/GUI effects) and rejects ungranted methods.
For every view command: open, submit its first submit-form action with the
view's own field values, refresh, dispose. Then shutdown with exit code 0.
"""
import argparse, hashlib, json, os, queue, stat, subprocess, tempfile, threading, uuid, zipfile
from pathlib import Path, PurePosixPath

STUB_REPLIES = {
    'host.clipboard.read': 'smoke text for the packaged executable',
    'host.selection.read': 'smoke text for the packaged executable',
    'host.clipboard.write': {'ok': True},
}


def granted(method, grants):
    short = method.removeprefix('host.')
    return method in grants or short in grants or short.split('.')[0] in grants


def form_values(node, values):
    props = node.get('props') or {}
    if node.get('type', '').startswith('Form.') and 'id' in props and 'value' in props:
        values[props['id']] = props['value']
    for child in node.get('children') or []:
        form_values(child, values)
    return values


def smoke(path, host_root=None):
    with tempfile.TemporaryDirectory() as temp, zipfile.ZipFile(path) as archive:
        root = Path(temp) / 'package'
        root.mkdir()
        names = archive.namelist()
        assert len(names) == len(set(names)), 'Duplicate ZIP paths'
        for item in archive.infolist():
            name = PurePosixPath(item.filename)
            assert not name.is_absolute() and '..' not in name.parts and '\\' not in item.filename and ':' not in item.filename, 'Unsafe ZIP path'
            assert not stat.S_ISLNK(item.external_attr >> 16), 'Symlink ZIP entry'
        manifest = json.loads(archive.read('extension.json'))
        inventory = manifest['files']
        assert set(names) == set(inventory) | {'extension.json'}, 'ZIP/inventory mismatch'
        for name, sha in inventory.items():
            assert hashlib.sha256(archive.read(name)).hexdigest() == sha, 'SHA inventory mismatch: ' + name
        for icon in [manifest.get('icon')] + [c.get('icon') for c in manifest['commands']]:
            assert icon is None or icon in inventory, f'Icon not packaged: {icon}'
        archive.extractall(root)
        slug = manifest['slug']
        binary = root / 'bin' / ('tezbar-' + slug + ('.exe' if os.name == 'nt' else ''))
        binary.chmod(0o755)
        env = {k: v for k, v in os.environ.items() if k in ('SystemRoot', 'WINDIR', 'TEMP', 'TMP')}
        env['PATH'] = os.path.join(env.get('SystemRoot', 'C:/Windows'), 'System32') if os.name == 'nt' else '/usr/bin:/bin'
        info = json.loads(subprocess.check_output([str(binary), '--protocol-info'], cwd=root, env=env, timeout=10))
        assert info['id'] == manifest['id'] and info['version'] == manifest['version']
        process = subprocess.Popen([str(binary)], cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=None)
        messages = queue.Queue()
        grants = manifest['capabilities']
        reverse = []

        def reader():
            try:
                while True:
                    line = process.stdout.readline(16 * 1024 * 1024 + 1)
                    if not line:
                        messages.put(EOFError('Unexpected EOF'))
                        return
                    if len(line) > 16 * 1024 * 1024:
                        raise ValueError('Oversized frame')
                    messages.put(json.loads(line))
            except Exception as error:
                messages.put(error)

        threading.Thread(target=reader, daemon=True).start()

        def receive():
            value = messages.get(timeout=8)
            if isinstance(value, Exception):
                raise value
            return value

        def send(value):
            process.stdin.write((json.dumps(value) + '\n').encode())
            process.stdin.flush()

        def request(method, params, allow_error=False):
            ident = 'h:' + str(uuid.uuid4())
            send({'type': 'request', 'id': ident, 'method': method, 'params': params})
            while True:
                frame = receive()
                if frame['type'] == 'response' and frame['id'] == ident:
                    if 'error' in frame and not allow_error:
                        raise AssertionError(frame)
                    return frame.get('result')
                if frame['type'] == 'request':
                    reverse.append(frame['method'])
                    assert granted(frame['method'], grants), 'Ungranted reverse RPC: ' + frame['method']
                    assert frame['method'] in STUB_REPLIES, 'No stub for ' + frame['method']
                    send({'type': 'response', 'id': frame['id'], 'result': STUB_REPLIES[frame['method']]})
                    continue
                assert frame['type'] in ('event', 'cancel'), frame

        try:
            hello = receive()
            assert hello['type'] == 'hello' and hello['id'] == manifest['id'] and hello['version'] == manifest['version'] and hello['protocol'] == 1
            request('initialize', {'protocol': 1, 'extensionId': manifest['id'], 'platform': 'x86_64-pc-windows-msvc' if os.name == 'nt' else 'aarch64-apple-darwin', 'packageDir': str(root), 'dataDir': str(Path(temp) / 'data'), 'tempDir': str(Path(temp) / 'temp'), 'grantedCapabilities': grants, 'reason': 'command', 'migrationPaths': {}})
            exercised = []
            for command in manifest['commands']:
                if command.get('mode') != 'view' or command['runtime'] != 'rust':
                    continue
                sid = 'smoke-' + command['name']
                view = request('session.open', {'sessionId': sid, 'commandName': command['name'], 'preferences': {}, 'argumentValues': {}})
                assert view['mode'] == 'view', view
                submit = next((a for a in view.get('actions', []) if a.get('kind') == 'submit-form'), None)
                if submit:
                    after = request('session.call', {'sessionId': sid, 'operation': 'extension:invoke-action', 'payload': {'actionId': submit['id'], 'formValues': form_values(view['root'], {})}})
                    assert after['mode'] in ('view', 'unchanged'), after
                    stale = request('session.call', {'sessionId': sid, 'operation': 'extension:invoke-action', 'payload': {'actionId': 'not-a-real-action'}}, allow_error=True)
                    assert stale is None, 'Unknown action must be rejected'
                request('session.call', {'sessionId': sid, 'operation': 'extension:refresh-session', 'payload': {}})
                request('session.dispose', {'sessionId': sid})
                exercised.append(command['name'])
            request('shutdown', {})
            assert process.wait(timeout=5) == 0, 'Process did not exit cleanly'
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            process.stdin.close()
            process.stdout.close()
        if host_root:
            subprocess.run(['node', str(Path(__file__).with_name('renderer-contract.cjs').resolve()),
                            str(host_root.resolve()), str(binary.resolve())], cwd=root, check=True, timeout=45)
        print(slug, 'ZIP inventory + protocol-info/hello/initialize + open/submit/stale-reject/refresh/dispose for',
              ', '.join(exercised), '+ shutdown passed; stub reverse RPCs:', ', '.join(reverse) or 'none', '(offline, no installation)')


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('archive', type=Path)
    p.add_argument('--host-root', type=Path, help='Read-only current host checkout for extracted-binary renderer regressions')
    a = p.parse_args()
    smoke(a.archive, a.host_root)
