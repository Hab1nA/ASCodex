#!/usr/bin/env python3
"""Inspect top ultrasound attempts' scoring structure."""
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
CID = "focused-imaging-and-resolution-characterisation-fr-e287fbca"

r = requests.get(f"{BASE}/challenges/{CID}/attempts", headers=H,
                 params={"limit": 100}, timeout=60)
items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
top = sorted([x for x in items if (x.get("score") or 0) >= 90],
             key=lambda x: -(x.get("score") or 0))
print("score>=90:", len(top))
for x in top[:5]:
    aid = x.get("id")
    d = requests.get(f"{BASE}/attempts/{aid}", headers=H, timeout=60).json()
    rj = d.get("resultsJson") or {}
    sd = d.get("scoringDetails") or {}
    print("=" * 70)
    print(f"{aid} score={d.get('score')} author={d.get('author_name')} "
          f"harness={d.get('harness')} model={d.get('modelTag')}")
    print("resultsJson keys:", list(rj.keys())[:12])
    print("harbor_reward:", rj.get("harbor_reward"), "trace_score:", rj.get("trace_score"),
          "scored_by:", rj.get("scored_by"))
    print("scoringDetails.source:", sd.get("source"))
    sc = d.get("scorecard") or {}
    print("scorecard:", json.dumps(sc))
