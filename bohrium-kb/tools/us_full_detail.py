#!/usr/bin/env python3
"""Full detail of a full-score ultrasound attempt."""
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

d = requests.get(f"{BASE}/attempts/23965", headers=H, timeout=60).json()
for k in ("id", "status", "score", "method", "detail", "modelTag", "harness",
          "scriptPath", "bundlePath", "bundleStatus", "execStatus", "execLog",
          "computeRequest", "cpuHours", "rawMessagesPath", "traceCount",
          "resultsJson", "scorecard", "scoringDetails", "createdAt", "updatedAt"):
    v = d.get(k)
    if v is None:
        continue
    print(f"{k}: {json.dumps(v, ensure_ascii=False, default=str)[:500]}")
