#!/usr/bin/env python3
"""List identities that still need the HUMAN token for cleanup:
  - redundant identities with remaining attempts (delete-attempts)
  - bound zero-attempt identities (unbind only)
"""
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
data = json.load(open(os.path.join(ROOT, "_logs", "roster_attempts.json"), encoding="utf-8"))

# challenge maxes (tie-aware keepers)
ch_max = {}
for label, items in data.items():
    for a in items:
        sc = a.get("score") or 0.0
        ch_max[label] = max(ch_max.get(label, 0.0), sc)
keepers = set()
for label, items in data.items():
    for a in items:
        if abs((a.get("score") or 0.0) - ch_max[label]) < 1e-6:
            keepers.add(str(a.get("authorId") or ""))

# remaining attempts per redundant identity
rem = {}
for label, items in data.items():
    for a in items:
        au = str(a.get("authorId") or "")
        if au in keepers:
            continue
        d = rem.setdefault(au, [0, []])
        d[0] += 1
        d[1].append((label, a.get("id"), a.get("score")))

print("REDUNDANT with remaining attempts (need human token to delete): "
      f"{len(rem)} identities, {sum(v[0] for v in rem.values())} attempts")
total = 0
for au, (n, lst) in sorted(rem.items(), key=lambda kv: (-kv[1][0], kv[0])):
    detail = ",".join(f"{l}:{aid}" for l, aid, sc in lst)
    total += n
    print(f"  {au:<22} attempts={n:<3} {detail}")
print("total remaining:", total)

# zero-attempt bound identities we know of (from local credentials)
print()
print("BOUND zero-attempt identity (unbind only, no attempts to delete): friday-n55379-n3")
print()
print("KEEPERS (not touched, stay):", sorted(keepers))
