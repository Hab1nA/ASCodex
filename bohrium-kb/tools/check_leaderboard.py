#!/usr/bin/env python3
"""Inspect the hackathon leaderboard: how are our identities presented?"""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

r = requests.get(f"{BASE}/leaderboard", headers=H, params={"hackathon": "true", "limit": 200}, timeout=60)
print("status:", r.status_code)
try:
    d = r.json()
except ValueError:
    print(r.text[:500])
    sys.exit(0)
if isinstance(d, dict):
    print("keys:", list(d.keys()))
    d = d.get("entries") or d.get("leaderboard") or d.get("results") or []
print("entries:", len(d))
for e in d[:60]:
    print({k: e.get(k) for k in ("name", "score", "complete", "model", "operatorName", "operatorId", "id", "authorId") if k in e})
