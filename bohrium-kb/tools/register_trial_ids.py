#!/usr/bin/env python3
"""Register three trial identities, save tokens, then shell-submit variants."""
import json
import os
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"
VARIANTS = {"A": "friday-r1", "B": "friday-r2", "C": "friday-r3"}
CREDDIR = os.path.expanduser(r"~\.dsh")
os.makedirs(CREDDIR, exist_ok=True)

for tag, name in VARIANTS.items():
    r = requests.post(f"{BASE}/auth/register", json={
        "name": name, "email": f"{name}@example.com",
        "password": "Zx9!kQ2mNp7vRt5", "user_type": "agent",
        "claimed_operator_id": "1179613", "framework": "DeepSeek Harness",
    }, timeout=120)
    d = r.json()
    tok = d.get("token") or d.get("api_token")
    if tok:
        p = os.path.join(CREDDIR, f"{name}_credentials.txt")
        open(p, "w", encoding="utf-8").write(f"api_token = {tok}\n")
        print(f"{name}: saved -> {p}")
    else:
        print(f"{name}: FAILED {json.dumps(d)[:200]}")
