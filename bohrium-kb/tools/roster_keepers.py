#!/usr/bin/env python3
"""Tie-aware keeper analysis: an identity is a keeper if its best score in some
challenge EQUALS the team's max score in that challenge (within 1e-6)."""
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
data = json.load(open(os.path.join(ROOT, "_logs", "roster_attempts.json"), encoding="utf-8"))

# per challenge: team max score + all identities achieving it
ch_best = {}
for label, items in data.items():
    best = 0.0
    for a in items:
        sc = a.get("score") or 0.0
        if sc > best:
            best = sc
    tied = set()
    for a in items:
        sc = a.get("score") or 0.0
        if abs(sc - best) < 1e-6:
            tied.add(str(a.get("authorId") or ""))
    ch_best[label] = (best, tied)

print("PER-CHALLENGE max + ALL identities tied at max:")
for label, (best, tied) in ch_best.items():
    print(f"{label:<12} max={best:>7.3f}  tied={sorted(tied)}")

keepers = {}
for label, (best, tied) in ch_best.items():
    for aid in tied:
        keepers.setdefault(aid, []).append(label)

print()
print("KEEPERS (tie-aware):", json.dumps(keepers, indent=1, ensure_ascii=False))

# roster with best-per-challenge per identity
per_id = {}
for label, items in data.items():
    for a in items:
        aid = str(a.get("authorId") or "")
        sc = a.get("score") or 0.0
        d = per_id.setdefault(aid, {})
        d[label] = max(d.get(label, 0.0), sc)

print()
print("REDUNDANT (no challenge max, tie-aware):")
for aid in sorted(per_id):
    if aid not in keepers:
        bests = {k: round(v, 3) for k, v in sorted(per_id[aid].items())}
        print(f"{aid:<24} per-challenge-bests={bests}")

print()
print("Identities with attempts: {0}".format(len(per_id)))
