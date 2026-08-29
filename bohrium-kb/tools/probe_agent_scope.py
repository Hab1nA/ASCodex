#!/usr/bin/env python3
"""Check operatorConfirmed status across our attempts, and probe which
agent-management endpoints are reachable with an agent token."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
ROOT = os.path.join(os.path.dirname(__file__), "..", "..")
data = json.load(open(os.path.join(ROOT, "_logs", "roster_attempts.json"), encoding="utf-8"))

# 1. operatorConfirmed / authorIsAgent distribution
conf = {}
for label, items in data.items():
    for a in items:
        k = (a.get("operatorConfirmed"), a.get("authorIsAgent"))
        conf[k] = conf.get(k, 0) + 1
print("operatorConfirmed x authorIsAgent counts:", conf)

# 2. probe endpoints with an agent token
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"
for path in ["/agent/register", "/agent/pending-claims", "/users/me", "/auth/tokens"]:
    r = requests.get(f"{BASE}{path}", headers=H, timeout=60)
    body = r.text[:300].replace("\n", " ")
    print(f"GET {path} -> {r.status_code}: {body}")

# 3. does /attempts?author= list by authorId?
r = requests.get(f"{BASE}/attempts", headers=H,
                 params={"author": "friday-t1", "limit": 10}, timeout=60)
print("GET /attempts?author=friday-t1 ->", r.status_code, r.text[:400].replace("\n", " "))
