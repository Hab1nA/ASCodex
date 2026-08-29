#!/usr/bin/env python3
"""GitHub search for manifold flow distribution solvers."""
import json
import sys
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")

for q in ["manifold flow distribution", "bajura jones", "header flow divider", "z-manifold flow", "dividing manifold header"]:
    url = "https://api.github.com/search/repositories?q=" + urllib.parse.quote(q) + "&per_page=6"
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            d = json.load(r)
    except Exception as e:
        print("===", q, "ERR", e)
        continue
    print("===", q)
    for it in d.get("items", []):
        line = f"  {it['full_name']:55s} {it.get('stargazers_count')}* {(it.get('description') or '')[:80]}"
        print(line)
