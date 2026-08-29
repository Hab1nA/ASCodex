#!/usr/bin/env python3
"""Decision table for the 15 new S4 challenges."""
import json, os, re, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

base = os.path.dirname(os.path.abspath(__file__))
out = json.load(open(os.path.join(base, 'new_s4_challenges.json'), encoding='utf-8'))

# sort by attempts (popularity) desc, then difficulty asc
out.sort(key=lambda c: (-c.get('attempts', 0), c.get('difficulty', 9), c['_id']))

buf = []
def P(s=''):
    buf.append(s)
for c in out:
    P("=" * 100)
    P(f"ID:     {c['_id']}")
    P(f"TITLE:  {c.get('title')}")
    P(f"diff={c.get('difficulty')}  disc={c.get('disc')}  attempts={c.get('attempts')}  stars={c.get('stars')}  status={c.get('status')}")
    sc = c.get('scoring')
    if isinstance(sc, dict):
        P(f"scoring keys: {list(sc.keys())}")
    P(f"round: seq={c.get('roundSeq')} end={c.get('roundEndAt')}")
    ab = (c.get('abstract') or '')[:400].replace('\n', ' ')
    P(f"ABSTRA: {ab}")
    tags = c.get('tags')
    P(f"tags: {tags}")
text = "\n".join(buf)
open(os.path.join(base, 'decision_table.txt'), 'w', encoding='utf-8').write(text)
print(text)
