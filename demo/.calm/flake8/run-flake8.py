#!/usr/bin/env python
import os
import re
import sys
import json
import subprocess

_report_re = re.compile(r'''(?x)
    (?P<filename>[^:]+):
    (?P<line>\d+):
    (?P<column>\d+):\s
    (?P<code>\S+)\s
    (?P<message>.*)
''')

args = []

for arg in sys.argv[1:]:
    args.append('--filename=%s' % os.path.join('.', arg))

c = subprocess.Popen(['flake8'] + args, stdout=subprocess.PIPE)

while 1:
    line = c.stdout.readline()
    if not line:
        break
    match = _report_re.match(line)
    if match is not None:
        d = match.groupdict()
        print(json.dumps({
            'filename': d['filename'],
            'line': int(d['line']),
            'column': int(d['column']),
            'code': d['code'],
            'message': d['message'],
        }))

c.wait()
