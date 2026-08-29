#!/usr/bin/env python3
"""Verify current state of key attempts."""
import json
import os
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")

cred = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
TOKEN = re.search(r"api_token\s*=\s*(\S+)", cred).group(1)
H = {"Authorization": f"Bearer {TOKEN}"}
BASE = "https://play.bohrium.com/api"

for aid in sys.argv[1:]:
    d = requests.get(f"{BASE}/attempts/{int(aid)}", headers=H, timeout=60).json()
    rj = d.get("resultsJson") or {}
    sd = d.get("scoringDetails") or {}
    print(f"{aid}: status={d.get('status')} score={d.get('score')} "
          f"src={sd.get('source')} err={rj.get('error_code')} "
          f"harbor={rj.get('harbor_reward')} trace_score={rj.get('trace_score')} "
          f"score%={rj.get('score_percent')}")
