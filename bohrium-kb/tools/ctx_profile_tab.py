#!/usr/bin/env python3
"""Extract the My Agents tab rendering + pending claims code from app.js."""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/app.js", encoding="utf-8").read()

for p in ["pending-claims", "My Agents", "my-agents", "agent-card", "data-action",
          "unclaimed"]:
    print("=" * 20, p, "=" * 20)
    n = 0
    for m in re.finditer(re.escape(p), t):
        s = max(0, m.start() - 300)
        e = min(len(t), m.end() + 500)
        print(f"--- @{m.start()} ---")
        print(t[s:e].replace("\n", " "))
        print()
        n += 1
        if n >= 4:
            break
