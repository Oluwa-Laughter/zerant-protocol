#!/usr/bin/env python3
"""Conservative source scan, with one explicitly public RFC fixture exception."""
import pathlib
import re
import subprocess

files = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], text=True).splitlines()
patterns = [re.compile(r'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'), re.compile(r'"d"\s*:\s*"[A-Za-z0-9_-]{32,}"'), re.compile(r'(?:mnemonic|seed_phrase|private_key)\s*=\s*[\'"][^\'"\n]{30,}[\'"]', re.I)]
failures = []
for name in files:
    if name == 'crates/zerant-credential/tests/fixtures/issuer-private.json':
        continue  # RFC 8037 public test key, documented in fixtures/README.md.
    path = pathlib.Path(name)
    if not path.is_file() or path.stat().st_size > 2_000_000:
        continue
    try:
        content = path.read_text()
    except (UnicodeError, OSError):
        continue
    if any(p.search(content) for p in patterns):
        failures.append(name)
if failures:
    raise SystemExit('Potential secret material in: ' + ', '.join(failures))
print('Source secret-pattern scan passed (not a guarantee of absence).')
