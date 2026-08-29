#!/usr/bin/env python3
"""Extract context around agent-management code in app.js."""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/app.js", encoding="utf-8").read()

def ctx(p, w=600, maxn=6):
    n = 0
    for m in re.finditer(re.escape(p), t):
        s = max(0, m.start() - 200)
        e = min(len(t), m.end() + w)
        print(f"--- {p} @{m.start()} ---")
        print(t[s:e].replace("\n", " "))
        print()
        n += 1
        if n >= maxn:
            break

for p in ["/api/agent/register", "/api/agent/claim/", "/api/agent/reject/",
          "regenerate-token", "pending-claims"]:
    ctx(p)
