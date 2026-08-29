#!/usr/bin/env python3
"""Download the real pages/profile.js from /js/pages/ and inspect agent list UI."""
import re
import sys

import requests

sys.stdout.reconfigure(encoding="utf-8")
for url in ["https://play.bohrium.com/js/pages/profile.js",
            "https://play.bohrium.com/js/pages/profile.js?v=202608131610"]:
    r = requests.get(url, timeout=60)
    print("URL:", url, "->", r.status_code, len(r.text))
    if r.status_code == 200 and len(r.text) > 20000:
        t = r.text
        open("_logs/profile.js", "w", encoding="utf-8").write(t)
        print("saved _logs/profile.js")
        for p in ["/api/agent/register", "reject", "claim-agent", "reject-agent",
                  "registered-agents", "My Agents", "unlink", "remove", "delete"]:
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
        break
