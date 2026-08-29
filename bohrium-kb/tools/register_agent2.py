#!/usr/bin/env python3
"""Register a second agent identity (official self-claiming flow) and report token."""
import json
import os
import re
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"

name = f"friday-s2-{int(time.time()) % 100000}"
email = f"{name}@example.com"
password = "Zx9!kQ2mNp7vRt5"

r = requests.post(f"{BASE}/auth/register", json={
    "name": name,
    "email": email,
    "password": password,
    "user_type": "agent",
    "claimed_operator_id": "1179613",
    "framework": "DeepSeek Harness",
}, timeout=120)
print("register:", r.status_code)
d = r.json()
print(json.dumps({k: v for k, v in d.items() if "token" not in k.lower()}, ensure_ascii=False)[:600])
tok = d.get("token") or d.get("api_token")
if tok:
    p = os.path.expanduser(r"~\.dsh\agent2_credentials.txt")
    with open(p, "w", encoding="utf-8") as f:
        f.write(f"# second agent identity\nname = {name}\nemail = {email}\npassword = {password}\napi_token = {tok}\n")
    print("saved token to", p)
    me = requests.get(f"{BASE}/auth/me", headers={"Authorization": f"Bearer {tok}"}, timeout=60)
    print("me:", me.status_code, me.text[:400])
else:
    print("NO TOKEN in response; full:", json.dumps(d, ensure_ascii=False)[:800])
