#!/usr/bin/env python3
"""Fetch challenge content (markdown guide) for candidate ids, save under _research/round4/."""
import os
import re
import sys
from pathlib import Path

import requests

BASE = "https://play.bohrium.com/api"
CRED = os.path.expanduser("~/.dsh/bohrium_credentials.txt")
OUT = Path(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\_research\round4\content")
OUT.mkdir(parents=True, exist_ok=True)

txt = Path(CRED).read_text(encoding="utf-8")
tok = re.search(r"api_token\s*=\s*(\S+)", txt).group(1)
h = {"Authorization": f"Bearer {tok}"}

IDS = sys.argv[1:] or [
    "dft-crystal-structure-formation-energy-and-charge-730d57d3",
]
for cid in IDS:
    r = requests.get(BASE + f"/challenges/{cid}/content", headers=h, timeout=120)
    p = OUT / (cid + ".md")
    p.write_text(r.text, encoding="utf-8")
    print(f"{cid} -> {p} ({len(r.text)} chars, http {r.status_code})")

