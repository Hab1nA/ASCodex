#!/usr/bin/env python3
"""Dump full resultsJson for agent2 attempts."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\agent2_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

for aid in sys.argv[1:]:
    d = requests.get(f"{BASE}/attempts/{int(aid)}", headers=H, timeout=60).json()
    print("=" * 90)
    print(aid, "status:", d.get("status"), "score:", d.get("score"))
    rj = d.get("resultsJson") or {}
    print(json.dumps(rj, ensure_ascii=False, default=str)[:3500])
