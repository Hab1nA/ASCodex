#!/usr/bin/env python3
"""Download app.js bundle and grep for agent-management API calls."""
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
url = "https://play.bohrium.com/js/app.js?v=202608131610"
r = requests.get(url, timeout=120)
print("status:", r.status_code, "len:", len(r.text))
open("_logs/app.js", "w", encoding="utf-8").write(r.text)

patterns = [
    r'.{80}agent/register.{120}',
    r'.{60}/agent/(claim|reject|pending-claims).{100}',
    r'.{60}(deleteAgent|removeAgent|unbindAgent|delete_agent|remove_agent).{100}',
    r'.{80}DELETE.{0,40}agent.{120}',
    r'.{60}method:\s*["\']DELETE["\'].{0,200}',
]
seen = set()
for p in patterns:
    for m in re.finditer(p, r.text):
        s = m.group(0)
        if s not in seen:
            seen.add(s)
            print("----", p[:40])
            print(s.replace("\n", " ")[:400])
