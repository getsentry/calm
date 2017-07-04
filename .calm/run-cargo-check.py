#!/usr/bin/env python
import re
import sys
import json
import subprocess

_warn_re = re.compile(r'#\[warn\(([^)]+)\)\]')


c = subprocess.Popen(
    ['cargo', 'check', '--message-format=json'],
    stdout=subprocess.PIPE)

while 1:
    line = c.stdout.readline()
    if not line:
        break
    d = json.loads(line)
    msg = d.get('message')
    reason = d.get('reason')
    spans = msg and msg.get('spans')
    if msg and reason and spans:
        message = msg['message']
        if spans[0].get('label'):
            message = '%s (%s)' % (message, spans[0]['label'])
        code = (msg.get('code') or {}).get('code')
        if code is None and msg.get('children'):
            for child in msg['children']:
                match = _warn_re.search(child['message'])
                if match is not None:
                    code = 'warning-%s' % match.group(1).replace('_', '-')
                    break
        print(json.dumps({
            'filename': spans[0]['file_name'],
            'code': code or 'generic',
            'level': msg['level'],
            'message': message,
            'line': spans[0]['line_start'],
            'column': spans[0]['column_start'],
        }))

sys.exit(c.wait())
