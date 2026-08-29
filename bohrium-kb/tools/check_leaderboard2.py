#!/usr/bin/env python3
"""Inspect full leaderboard entry structure + locate our team's entries."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

r = requests.get(f"{BASE}/leaderboard", headers=H, params={"hackathon": "true", "limit": 500}, timeout=60)
d = r.json()
print("entry keys:", sorted(d[0].keys()) if d else "empty")
print()
import json
for e in d:
    name = str(e.get("name") or "")
    if any(k in name.lower() for k in ("friday", "谢铠舟", "1179613", "jarvis", "ultron", "tianhan")):
        print(json.dumps(e, ensure_ascii=False))
print()
# also check per-challenge leaderboard for twist
r2 = requests.get(f"{BASE}/leaderboard", headers=H,
                  params={"challenge_id": "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd", "limit": 30}, timeout=60)
try:
    d2 = r2.json()
    print("twist leaderboard:", r2.status_code)
    for e in (d2[:30] if isinstance(d2, list) else []):
        print({k: e.get(k) for k in ("name", "score", "authorId", "operatorName") if k in e})
except ValueError:
    print("twist lb parse fail:", r2.text[:200])
