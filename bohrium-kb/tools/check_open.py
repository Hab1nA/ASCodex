#!/usr/bin/env python3
"""Is this challenge currently open for submission? Check status + newest attempts."""
import json
import os
import re
from pathlib import Path

import requests

cred = os.path.expanduser("~/.dsh/bohrium_credentials.txt")
tok = re.search(r"api_token\s*=\s*(\S+)", Path(cred).read_text(encoding="utf-8")).group(1)
h = {"Authorization": f"Bearer {tok}"}
BASE = "https://play.bohrium.com/api"
cid = "estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7"
c = requests.get(BASE + f"/challenges/{cid}", headers=h, timeout=60).json()
print("status:", c.get("status"), "| roundId:", c.get("roundId"), "| roundSeq:", c.get("roundSeq"))
print("roundEndAt:", c.get("roundEndAt"), "| roundStartAt:", c.get("roundStartAt"))
r = requests.get(BASE + f"/challenges/{cid}/attempts", headers=h, params={"sort": "newest", "limit": 8}, timeout=60).json()
items = r if isinstance(r, list) else (r.get("items") or [])
for a in items:
    print(f"newest: id={a['id']} created={a.get('createdAt')} status={a.get('status')} score={a.get('score')}")
d = json.load(open(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\_logs\cand_details.json", encoding="utf-8"))
for x in d:
    if x.get("id") == cid:
        print("cand_details status:", x.get("status"), "| attempts field:", x.get("attempts"))
