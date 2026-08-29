#!/usr/bin/env python3
"""Find recent friday attempts on a challenge across all pages."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
slug = sys.argv[1]
page = 1
fetched = 0
total = 0
while True:
    r = requests.get(
        f"https://play.bohrium.com/api/challenges/{slug}/attempts",
        params={"per_page": 20, "page": page}, headers=H, timeout=60)
    d = r.json() or {}
    items = d.get("attempts") or []
    total = d.get("total", 0)
    if not items:
        break
    fetched += len(items)
    for a in items:
        au = str(a.get("authorId") or "")
        if "friday" in au.lower():
            print(f"aid={a.get('id')} author={au} status={a.get('status')} "
                  f"score={a.get('score')} at={a.get('createdAt')}")
    page += 1
    if fetched >= total or page > 40:
        break
