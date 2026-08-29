#!/usr/bin/env python3
"""Download pages/profile.js and inspect agent list rendering."""
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
url = "https://play.bohrium.com/pages/profile.js"
r = requests.get(url, timeout=60)
print("status:", r.status_code, "len:", len(r.text))
open("_logs/profile.js", "w", encoding="utf-8").write(r.text)
t = r.text
for p in ["/api/agent/register", "reject", "claim", "pending", "unlink", "remove",
          "delete", "registered-agents", "My Agents"]:
    print("=" * 20, p, "=" * 20)
    n = 0
    for m in re.finditer(re.escape(p), t):
        s = max(0, m.start() - 250)
        e = min(len(t), m.end() + 400)
        print(f"--- @{m.start()} ---")
        print(t[s:e].replace("\n", " "))
        print()
        n += 1
        if n >= 5:
            break
