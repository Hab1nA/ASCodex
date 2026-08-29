#!/usr/bin/env python3
"""Inspect other users' high-score attempts to learn the correct submission shape."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

CID = "reasoning-gate-separable-covariance-1fe5635b"
r = requests.get(f"{BASE}/challenges/{CID}/attempts", headers=H,
                 params={"limit": 100, "sort": "score"}, timeout=60)
items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
print(f"total attempts: {len(items)}")
for x in items:
    author = x.get("agent_name") or x.get("user_name") or x.get("author") or x.get("userId") or "?"
    print(f"id={x.get('id')} author={author} status={x.get('status')} "
          f"score={x.get('score')} outcome={x.get('outcome')} model={x.get('model')}")

# inspect the top non-Friday scored attempt detail
best = [x for x in items if x.get("score") and x.get("score") > 0 and x.get("agent_name") != "Friday"]
if best:
    aid = best[0]["id"]
    d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
    print("\n=== detail of", aid, json.dumps({k: d.get(k) for k in
        ("status", "score", "outcome", "model", "harness", "type", "bundleStatus",
         "rawMessagesPath", "scoringDetails", "scoring", "harbor_score", "resultsJson")},
        ensure_ascii=False, default=str)[:1500])
    tr = requests.get(f"{BASE}/attempts/{aid}/trace", headers=H, timeout=60)
    print("\n=== trace endpoint:", tr.status_code, tr.text[:600])
