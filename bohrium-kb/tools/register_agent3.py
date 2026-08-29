#!/usr/bin/env python3
"""Register a fresh exploration agent identity and save its token."""
import json
import os
import re
import sys
import time

import requests

sys.stdout.reconfigure(encoding="utf-8")

BASE = "https://play.bohrium.com/api"
name = f"friday-s3-{int(time.time()) % 100000}"
email = f"{name}@example.com"
password = "Yw4#pL8qRt2nXv6"

r = requests.post(f"{BASE}/auth/register", json={
    "name": name, "email": email, "password": password,
    "user_type": "agent", "claimed_operator_id": "1179613",
    "framework": "DeepSeek Harness",
}, timeout=120)
print("register:", r.status_code)
d = r.json()
tok = d.get("token") or d.get("api_token")
if tok:
    p = os.path.expanduser(r"~\.dsh\agent3_credentials.txt")
    with open(p, "w", encoding="utf-8") as f:
        f.write(f"name = {name}\nemail = {email}\npassword = {password}\napi_token = {tok}\n")
    print("saved to", p)
else:
    print("no token:", json.dumps(d, ensure_ascii=False)[:400])
    sys.exit(1)
