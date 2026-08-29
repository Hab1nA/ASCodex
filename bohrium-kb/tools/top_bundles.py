#!/usr/bin/env python3
"""Find top attempts per challenge and test bundle download."""
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

for cid, label in [
    ("reasoning-gate-gbsde-feynman-kac-e8970329", "gbsde"),
    ("mp-r-mp-r-ab-uv-split-coann-6924985d", "split"),
    ("multi-sample-cnv-detection-from-binned-read-counts-15924b97", "cnv"),
]:
    r = requests.get(f"{BASE}/challenges/{cid}/attempts", headers=H,
                     params={"limit": 100}, timeout=60)
    items = r.json() if isinstance(r.json(), list) else r.json().get("attempts", [])
    scored = sorted([x for x in items if (x.get("score") or 0) > 50],
                    key=lambda x: -(x.get("score") or 0))
    print(f"=== {label}: top attempts")
    for x in scored[:4]:
        who = x.get("author_name") or x.get("userId") or x.get("authorId") or "?"
        print(f"   {x.get('id')} {who} score={x.get('score')} status={x.get('status')}")
    if scored:
        aid = scored[0]["id"]
        b = requests.get(f"{BASE}/attempts/{aid}/bundle", headers=H, timeout=60)
        print(f"   bundle download for {aid}: {b.status_code} {len(b.content)} bytes")
        if b.status_code == 200:
            import zipfile, io
            z = zipfile.ZipFile(io.BytesIO(b.content))
            names = [n for n in z.namelist() if "answer" in n or "outputs" in n][:10]
            print("   entries:", names)
