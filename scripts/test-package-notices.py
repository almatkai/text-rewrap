#!/usr/bin/env python3
"""End-to-end release gate regressions, restoring every temporary mutation."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/aarch64-apple-darwin/release/tezbar-text-rewrap'
index = json.loads((ROOT / 'licenses/dependency-license-index.json').read_text())
windows_only = next(item for item in index['dependencies'] if item['targets'] == ['x86_64-pc-windows-msvc'])
cases = [
    (next(iter(windows_only['documents'])), 'corrupt'),
    ('licenses/dependency-license-index.json', 'corrupt'),
    ('licenses/tezbar-extension-sdk-NOTICE.txt', 'missing'),
]
for relative, mutation in cases:
    document = ROOT / relative
    original = document.read_bytes()
    try:
        if mutation == 'missing':
            document.unlink()
        else:
            document.write_bytes(original + b'\ninvalid inventory test\n')
        with tempfile.TemporaryDirectory(dir=ROOT / 'target', prefix='notice-gate-') as temp:
            output = Path(temp) / 'output'
            run = subprocess.run([
                sys.executable, str(ROOT / 'scripts/package.py'),
                '--target', 'aarch64-apple-darwin', '--binary', str(BINARY),
                '--output', str(output),
            ], cwd=ROOT, capture_output=True, text=True, timeout=60)
            assert run.returncode != 0, 'Packaging accepted ' + relative
            assert not list(output.glob('*.zip')), 'ZIP written despite failed notice inventory'
            assert 'vendor-licenses.py' in run.stderr, run.stderr
            print('PASS: Mac package rejected ' + mutation + ' ' + relative + ' before ZIP creation')
    finally:
        document.write_bytes(original)
subprocess.run([sys.executable, str(ROOT / 'scripts/vendor-licenses.py'), '--check'], cwd=ROOT, check=True)
