#!/usr/bin/env python3
"""Show non-friday attempts (latest page) for a challenge."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
slug = sys.argv[1]
r = requests.get(
    f"https://play.bohrium.com/api/challenges/{slug}/attempts",
    params={"per_page": 20, "page": 1}, headers=H, timeout=60)
items = (r.json() or {}).get("attempts") or []
for a in items:
    au = str(a.get("authorId") or "")
    if "friday" in au.lower():
        continue
    print(f"aid={a.get('id')} author={au[:24]:24s} status={a.get('status')} "
          f"score={a.get('score')} outcome={a.get('outcome')} at={(a.get('createdAt') or '')[5:16]}")
