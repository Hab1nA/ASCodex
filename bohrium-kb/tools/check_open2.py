#!/usr/bin/env python3
"""DFT LiSi: latest attempt timestamps + full resources block."""
import json
import os
import re
from datetime import datetime, timezone
from pathlib import Path

import requests

print("now (local):", datetime.now().isoformat(), "| now (UTC):", datetime.now(timezone.utc).isoformat())

cred = os.path.expanduser("~/.dsh/bohrium_credentials.txt")
tok = re.search(r"api_token\s*=\s*(\S+)", Path(cred).read_text(encoding="utf-8")).group(1)
h = {"Authorization": f"Bearer {tok}"}
BASE = "https://play.bohrium.com/api"
cid = "dft-crystal-structure-formation-energy-and-charge-730d57d3"
r = requests.get(BASE + f"/challenges/{cid}/attempts", headers=h, params={"sort": "newest", "limit": 10}, timeout=60).json()
items = r if isinstance(r, list) else (r.get("items") or [])
print(f"\nlatest {len(items)} attempts on DFT LiSi:")
for a in items:
    print(f"  id={a['id']} created={a.get('createdAt')} status={a.get('status')} score={a.get('score')} author={a.get('author_name')}")

d = json.load(open(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\liSi-dft\challenge-dl\challenge.json", encoding="utf-8"))
print("\nresources:")
print(json.dumps(d["resources"], ensure_ascii=False, indent=2))
print("\nstatus:", d["status"], "| roundStart:", d["roundStartAt"], "| roundEnd:", d["roundEndAt"])
print("tags:", d["tags"], "| origin:", d["origin"], "| hackathonSeasonId:", d["hackathonSeasonId"])
