#!/usr/bin/env python3
"""Inspect twist challenge attempts incl. top scorers' method fields."""
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

cid = "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd"
r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H,
                 params={"limit": 100}, timeout=60)
items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
scored = sorted([x for x in items if (x.get("score") or 0) > 0],
                key=lambda x: -(x.get("score") or 0))
print(f"total attempts: {len(items)}, scored>0: {len(scored)}")
for x in scored[:8]:
    who = x.get("author_name") or x.get("userId") or x.get("authorId") or "?"
    print(f"\nid={x.get('id')} {who} score={x.get('score')} status={x.get('status')}")
    m = x.get("method") or ""
    print("  method:", m[:600])
