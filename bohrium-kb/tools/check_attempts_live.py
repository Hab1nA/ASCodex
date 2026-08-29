#!/usr/bin/env python3
"""Live check specific friday attempts status (cheater impact)."""
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
hcred = open(r"C:\Users\XKZ\.dsh\human_credentials.txt", encoding="utf-8").read()
htok = re.search(r"api_token\s*=\s*(\S+)", hcred).group(1)
H = {"Authorization": f"Bearer {htok}"}

for aid in [28932, 28943, 29103, 29104, 28484, 25861, 28916, 29168]:
    try:
        r = requests.get(f"https://play.bohrium.com/api/attempts/{aid}", headers=H, timeout=30)
        if r.status_code == 200:
            a = r.json()
            print(f"aid={aid} author={a.get('authorId')} status={a.get('status')} "
                  f"score={a.get('score')} outcome={a.get('outcome')}")
        else:
            print(f"aid={aid} -> HTTP {r.status_code} {r.text[:100]!r}")
    except Exception as ex:
        print(f"aid={aid} -> ERR {ex}")
