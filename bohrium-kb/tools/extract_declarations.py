#!/usr/bin/env python3
"""Extract per-identity declared (modelTag, harness) from attempt archives.

Sources:
  - round1 live archives: round1_prep/research/attempts_{03,09}_live.json,
    round1_prep/work/10-spatial/api_attempts.json
  - round3 fresh dump: _logs/roster_attempts.json (if present)
Output: per authorId -> distinct (modelTag, harness) combos + latest combo.
"""
import collections
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")

SRC = [
    "bohrium-kb/round1_prep/research/attempts_03_live.json",
    "bohrium-kb/round1_prep/research/attempts_09_live.json",
    "bohrium-kb/round1_prep/work/10-spatial/api_attempts.json",
    "_logs/roster_attempts.json",
]

per_author = collections.defaultdict(list)  # authorId -> list of (modelTag, harness, attempt_id)

for p in SRC:
    if not os.path.exists(p):
        print(f"[skip] {p}")
        continue
    data = json.load(open(p, encoding="utf-8"))
    if isinstance(data, dict) and "attempts" in data:
        data = data["attempts"]
    if isinstance(data, dict):  # roster format: label -> [attempts]
        items = []
        for v in data.values():
            items.extend(v if isinstance(v, list) else [v])
    else:
        items = data
    n = 0
    for a in items:
        if not isinstance(a, dict):
            continue
        aid = a.get("authorId") or a.get("author") or a.get("author_id")
        if not aid:
            continue
        mt = a.get("modelTag") or a.get("model") or a.get("model_id")
        h = a.get("harness") or a.get("agentFramework") or a.get("framework")
        per_author[aid].append((mt, h, a.get("id")))
        n += 1
    print(f"[ok] {p} -> {n} attempts")

print("\n=== per-identity declared (modelTag, harness) ===")
for aid in sorted(per_author):
    combos = collections.Counter((m, h) for m, h, _ in per_author[aid])
    latest = per_author[aid][-1][:2]
    print(f"{aid:<26} combos={dict(combos)} latest={latest}")
