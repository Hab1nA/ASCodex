#!/usr/bin/env python3
"""Judge signal card: score distribution + stuck attempts on KMC challenge."""
import json, os, sys, io, urllib.request
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

BASE = "https://play.bohrium.com/api"
CH = "estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7"
JWT = os.environ.get("FRIDAY_JWT", "")

def get(path):
    req = urllib.request.Request(BASE+path, headers={"Authorization": f"Bearer {JWT}"})
    with urllib.request.urlopen(req, timeout=90) as r:
        return json.loads(r.read().decode())

# attempts list (newest first), large limit
d = get(f"/challenges/{CH}/attempts?sort=newest&limit=200")
items = d if isinstance(d, list) else d.get("attempts", d.get("items", []))
print(f"total attempts listed: {len(items)}")
if not items:
    print("keys:", list(d.keys()) if isinstance(d, dict) else type(d))
    sys.exit(0)

# score distribution
from collections import Counter
scores = []
by_outcome = Counter()
by_author = Counter()
stuck = []
for a in items:
    s = a.get("score")
    if s is not None:
        scores.append(s)
    by_outcome[a.get("outcome")] += 1
    by_author[a.get("authorId") or a.get("author_name")] += 1
    if a.get("outcome") == "stuck":
        stuck.append(a)

print("\n=== score distribution ===")
if scores:
    scores_sorted = sorted(scores, reverse=True)
    print(f"count={len(scores)} max={max(scores)} min={min(scores)} mean={sum(scores)/len(scores):.2f}")
    print("top 25:", [round(s,2) for s in scores_sorted[:25]])
    # histogram in bands of 10
    hist = Counter(int(s//10)*10 for s in scores)
    for b in sorted(hist, reverse=True):
        print(f"  {b:3d}-{b+9:3d}: {'#'*min(hist[b],40)} ({hist[b]})")

print("\n=== by outcome ===")
for k,v in by_outcome.most_common():
    print(f"  {k}: {v}")

print("\n=== top 5 authors ===")
for k,v in by_author.most_common(5):
    print(f"  {k}: {v}")

print(f"\n=== stuck attempts: {len(stuck)} ===")
for a in stuck[:10]:
    print(f"  id={a.get('id')} author={a.get('authorId')} score={a.get('score')} "
          f"stuckAt={str(a.get('stuckAt'))[:80]}")
    print(f"     method={str(a.get('method'))[:100]}")

# sample the top-scoring attempt's scoringDetails if present
top = max((a for a in items if a.get("score") is not None), key=lambda a: a["score"], default=None)
if top:
    print(f"\n=== top attempt id={top['id']} score={top['score']} author={top.get('authorId')} ===")
    sd = top.get("scoringDetails")
    if sd:
        print("scoringDetails:", json.dumps(sd, ensure_ascii=False, indent=1)[:2000])
    sc = top.get("scorecard")
    if sc:
        print("scorecard:", json.dumps(sc, ensure_ascii=False, indent=1)[:800])
