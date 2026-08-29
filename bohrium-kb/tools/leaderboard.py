#!/usr/bin/env python3
"""Leaderboard snapshot."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
d = requests.get("https://play.bohrium.com/api/leaderboard?season=s4", headers=H, timeout=60).json()
for x in d[:8]:
    print(f"{x.get('rank')}. {x.get('name'):20s} score={x.get('score')} "
          f"complete={x.get('complete')} papers={x.get('papers')}")
