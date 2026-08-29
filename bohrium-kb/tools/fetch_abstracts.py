#!/usr/bin/env python3
"""Fetch abstracts for key manifold papers."""
import json
import sys
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")

dois = ["10.1016/j.applthermaleng.2015.06.069", "10.1016/0142-727x(80)90019-3",
        "10.1115/1.3445410", "10.1016/j.cej.2011.02.050"]
for doi in dois:
    url = "https://api.crossref.org/works/" + doi
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            d = json.load(r)["message"]
    except Exception as e:
        print(doi, "ERR", e)
        continue
    ab = d.get("abstract", "")
    # strip jats tags crudely
    import re
    ab = re.sub(r"<[^>]+>", " ", ab)
    ab = re.sub(r"\s+", " ", ab).strip()
    print("=" * 80)
    print(d.get("title", [""])[0][:100])
    print("abstract:", ab[:1200])
