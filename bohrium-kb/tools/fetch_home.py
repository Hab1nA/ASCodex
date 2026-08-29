#!/usr/bin/env python3
"""Fetch play.bohrium.com home page and list JS bundle URLs."""
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
r = requests.get("https://play.bohrium.com/", timeout=60)
print("status:", r.status_code, "len:", len(r.text))
open("_logs/play_home.html", "w", encoding="utf-8").write(r.text)
for m in sorted(set(re.findall(r'(?:src|href)="([^"]+\.js[^"]*)"', r.text))):
    print(m)
