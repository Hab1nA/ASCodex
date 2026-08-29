#!/usr/bin/env python3
import json, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
d = json.load(open('work/kmc-poi/arm_manifest_schema.json', encoding='utf-8'))
for k in ['arm_version', 'paper', 'entrypoint', 'entrypoint_args', 'environment',
          'execution', 'expected_outputs', 'characterization', 'trace', 'handoff',
          'skills_used', 'knowledge', 'rag', 'data_sources', 'provenance', 'scorecard']:
    v = d.get('properties', {}).get(k)
    print('=' * 70)
    print(k, '(required)' if k in d.get('required', []) else '')
    print(json.dumps(v, ensure_ascii=False, indent=1)[:1200])
