#!/usr/bin/env python3
"""Dump scorecards for attempts."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
for aid in [int(x) for x in sys.argv[1:]]:
    d = requests.get(f"https://play.bohrium.com/api/attempts/{aid}", headers=H, timeout=60).json()
    sc = d.get("scorecard") or {}
    print(f"aid={aid} challenge={str(d.get('challengeId'))[:32]} status={d.get('status')} "
          f"score={d.get('score')} outcome={d.get('outcome')}")
    print("   scorecard:", json.dumps(sc, default=str))
