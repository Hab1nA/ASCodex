#!/usr/bin/env python3
"""Show my latest attempts with scores for given challenges."""
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

for cid in sys.argv[1:]:
    r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H,
                     params={"limit": 100}, timeout=60)
    items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
    mine = [x for x in items if (x.get("authorId") == "friday" or x.get("author_name") == "Friday")]
    mine.sort(key=lambda x: x.get("id") or 0, reverse=True)
    print(f"=== {cid[:40]}: {len(mine)} attempts")
    for x in mine[:8]:
        print(f"  {x.get('id')} status={x.get('status')} score={x.get('score')} outcome={x.get('outcome')}")
