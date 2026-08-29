#!/usr/bin/env python3
"""Score-card recon: pull top attempts + score distribution + scorecards for a challenge.

Usage: python scorecard_recon.py [challenge_id] [challenge_id2 ...]
"""
import json
import os
import re
import statistics
import sys
from pathlib import Path

import requests

BASE = "https://play.bohrium.com/api"
CRED = os.path.expanduser("~/.dsh/bohrium_credentials.txt")
txt = Path(CRED).read_text(encoding="utf-8")
tok = re.search(r"api_token\s*=\s*(\S+)", txt).group(1)
h = {"Authorization": f"Bearer {tok}"}

cids = sys.argv[1:] or ["estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7"]
for cid in cids:
    print(f"\n############ {cid}")
    r = requests.get(BASE + f"/challenges/{cid}/attempts", headers=h, params={"sort": "score", "limit": 200}, timeout=120)
    body = r.json()
    items = body if isinstance(body, list) else (body.get("items") or body.get("attempts") or [])
    scored = [a for a in items if a.get("score") is not None]
    scores = sorted([a["score"] for a in scored], reverse=True)
    print(f"fetched: {len(items)} | scored: {len(scores)}")
    if scores:
        print(f"min={scores[-1]:.3f} median={statistics.median(scores):.3f} max={scores[0]:.4f}")
        buckets = {}
        for s in scores:
            k = int(s // 10) * 10
            buckets[k] = buckets.get(k, 0) + 1
        print("buckets:", sorted(buckets.items()))
    top5 = sorted(scored, key=lambda x: -(x.get("score") or 0))[:5]
    for a in top5:
        print(f"  id={a['id']} score={a['score']} author={a.get('author_name')} model={a.get('modelTag')} harness={a.get('harness')} outcome={a.get('outcome')} trace={a.get('traceCount')}")
        sc = a.get("scorecard") or {}
        if sc:
            print(f"    scorecard: {json.dumps(sc, ensure_ascii=False)[:400]}")
    # our attempts for this challenge
    ours = [a for a in items if str(a.get("authorId") or "").startswith("friday")]
    print(f"OUR attempts here: {len(ours)}")
    for a in ours:
        print(f"  ours: id={a['id']} score={a.get('score')} created={a.get('createdAt')}")
