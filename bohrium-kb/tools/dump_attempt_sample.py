#!/usr/bin/env python3
"""Dump one attempt's raw JSON to inspect field names."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
slug = "thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd"
r = requests.get(f"https://play.bohrium.com/api/challenges/{slug}/attempts",
                 params={"per_page": 5, "page": 1}, headers=H, timeout=60)
d = r.json()
print("top-level keys:", list(d.keys()) if isinstance(d, dict) else type(d))
items = d.get("attempts") or (d if isinstance(d, list) else [])
print("total:", d.get("total"))
if items:
    print("attempt keys:", sorted(items[0].keys()))
    import json as J
    print(J.dumps(items[0], ensure_ascii=False, indent=1)[:3000])
