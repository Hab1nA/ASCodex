#!/usr/bin/env python3
"""Find the registered-agents rendering code inside app.js."""
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
t = open("_logs/app.js", encoding="utf-8").read()

for p in ["_loadRegisteredAgents", "registered-agents-list", "renderProfile",
          "loadProfileForks", "switch-profile-tab", "profile-tab-panel"]:
    print("=" * 20, p, "=" * 20)
    n = 0
    for m in re.finditer(re.escape(p), t):
        s = max(0, m.start() - 200)
        e = min(len(t), m.end() + 700)
        print(f"--- @{m.start()} ---")
        print(t[s:e].replace("\n", " "))
        print()
        n += 1
        if n >= 3:
            break
