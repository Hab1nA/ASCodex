#!/usr/bin/env python3
"""Dump full content + scoring for a chosen challenge."""
import json, os, sys

base = os.path.dirname(os.path.abspath(__file__))
out = json.load(open(os.path.join(base, 'new_s4_challenges.json'), encoding='utf-8'))
want = sys.argv[1]
c = next(x for x in out if x['_id'] == want)
buf = []
def P(s=''): buf.append(s)
P(f"ID: {c['_id']}")
P(f"TITLE: {c.get('title')}")
P(f"status={c.get('status')} diff={c.get('difficulty')} disc={c.get('disc')} attempts={c.get('attempts')}")
P(f"roundEndAt={c.get('roundEndAt')} tags={c.get('tags')}")
P("\n" + "="*80 + "\nSCORING\n" + "="*80)
sc = c.get('scoring') or {}
if isinstance(sc, dict):
    for k, v in sc.items():
        P(f"\n--- {k} ---")
        if isinstance(v, (dict, list)):
            P(json.dumps(v, ensure_ascii=False, indent=1))
        else:
            P(str(v))
P("\n" + "="*80 + "\nCONTENT (topicContent/content)\n" + "="*80)
content = c.get('topicContent') or c.get('content') or ''
P(content)
text = "\n".join(buf)
open(os.path.join(base, 'candidate_full.txt'), 'w', encoding='utf-8').write(text)
print(f"wrote {len(text)} chars -> candidate_full.txt")
