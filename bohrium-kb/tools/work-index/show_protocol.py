#!/usr/bin/env python3
import json, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
d = json.load(open('work/protocol.json', encoding='utf-8'))
for k in ['status_machine', 'scorecard', 'submission_endpoint', 'trace_anti_fraud', 'tooling', 'name', 'version', 'reference_url', 'schemas']:
    print('=' * 70)
    print(k)
    print(json.dumps(d.get(k), ensure_ascii=False, indent=1))
