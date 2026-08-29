#!/usr/bin/env python3
"""Probe skill spec endpoints and list local skills properly."""
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com"

# local skills (correct path resolution)
local_dir = os.path.expanduser(r"~\.dsh\skills")
local = set(os.listdir(local_dir)) if os.path.isdir(local_dir) else set()
print("LOCAL .dsh/skills count:", len(local))
print("local:", sorted(local))

# probe spec endpoints on one skill
for ep in ("/api/skills/reproduce-paper/spec",
           "/api/skills/reproduce-paper",
           "/api/skills/reproduce-paper/download",
           "/api/skills/reproduce-paper.md"):
    try:
        r = requests.get(BASE + ep, headers=H, timeout=30)
        ct = r.headers.get("content-type", "")
        print(ep, r.status_code, ct, r.text[:120].replace("\n", " "))
    except Exception as e:
        print(ep, "ERR", e)
