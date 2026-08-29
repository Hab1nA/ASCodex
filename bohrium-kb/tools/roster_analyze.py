#!/usr/bin/env python3
"""Analyze roster_attempts.json:
  - identity roster under operator 1179613
  - per challenge: identity with the top score (the 'keeper' identity)
  - identities holding no top score anywhere -> redundant candidates
"""
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
data = json.load(open(os.path.join(ROOT, "_logs", "roster_attempts.json"), encoding="utf-8"))

# ---- 1. identity roster
roster = {}
for label, items in data.items():
    for a in items:
        aid = str(a.get("authorId") or "")
        if not aid:
            continue
        r = roster.setdefault(aid, {"attempts": 0, "scored": 0, "challenges": set(),
                                    "best_score": 0.0, "best_harbor": 0.0, "best_at": None})
        r["attempts"] += 1
        r["challenges"].add(label)
        sc = a.get("score") or 0.0
        if a.get("status") == "scored" or sc > 0:
            r["scored"] += 1
            sd = a.get("scorecard") or {}
            harbor = sd.get("harbor_reward")
            if harbor is None:
                harbor = sd.get("harbor_score") or 0.0
            if sc > r["best_score"] or (sc == r["best_score"] and (harbor or 0) > r["best_harbor"]):
                r["best_score"] = sc
                r["best_harbor"] = harbor or 0.0
                r["best_at"] = (label, a.get("id"))

print("=" * 100)
print("IDENTITY ROSTER (operator 1179613), from attempts: "
      f"{len(roster)} distinct authorIds, {sum(r['attempts'] for r in roster.values())} attempts")
print("=" * 100)
for aid, r in sorted(roster.items(), key=lambda kv: (-kv[1]["best_score"], kv[0])):
    chs = ",".join(sorted(r["challenges"]))
    print(f"{aid:<24} attempts={r['attempts']:<3} scored={r['scored']:<3} "
          f"best={r['best_score']:>7.3f} harbor={r['best_harbor']:>6.3f} "
          f"challs={chs:<42} bestAt={r['best_at']}")

# ---- 2. per-challenge top
print()
print("=" * 100)
print("PER-CHALLENGE: team top score & holder (the keepers)")
print("=" * 100)
keepers = {}
for label, items in data.items():
    best = None
    for a in items:
        sc = a.get("score") or 0.0
        sd = a.get("scorecard") or {}
        harbor = sd.get("harbor_reward")
        if harbor is None:
            harbor = sd.get("harbor_score") or 0.0
        key = (sc, harbor)
        if best is None or key > best[0]:
            best = (key, str(a.get("authorId") or ""), a.get("id"))
    (sc, harbor), aid, aid2 = best
    keepers.setdefault(aid, []).append(label)
    print(f"{label:<12} top={sc:>7.3f} harbor={harbor:>6.3f} aid={aid2} by={aid}")

print()
print("=" * 100)
print("HOLDERS of at least one challenge top score:")
print("=" * 100)
for aid, labels in sorted(keepers.items()):
    print(f"{aid:<24} -> {labels}")

# ---- 3. redundant identities
print()
print("=" * 100)
print("REDUNDANT CANDIDATES: never hold a challenge top score")
print("=" * 100)
for aid, r in sorted(roster.items()):
    if aid not in keepers:
        print(f"{aid:<24} best={r['best_score']:>7.3f} scored={r['scored']} "
              f"attempts={r['attempts']} challs={','.join(sorted(r['challenges']))}")
