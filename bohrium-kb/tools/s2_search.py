#!/usr/bin/env python3
"""Semantic Scholar search for Bajura-Jones and manifold flow calibration papers."""
import json
import sys
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")

def search(q, n=6):
    url = "https://api.semanticscholar.org/graph/v1/paper/search?query=" + urllib.parse.quote(q) + \
          f"&limit={n}&fields=title,year,authors,abstract,externalIds,openAccessPdf"
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "research-agent"})
        with urllib.request.urlopen(req, timeout=60) as r:
            d = json.load(r)
    except Exception as e:
        print("ERR", q, e)
        return
    print("===", q)
    for p in d.get("data", []):
        auth = ", ".join(a["name"] for a in p.get("authors", [])[:3])
        pdf = (p.get("openAccessPdf") or {}).get("url") or "-"
        print(f"  [{p.get('year')}] {p['title'][:90]}")
        print(f"     {auth} | {pdf[:100]}")
        ab = (p.get("abstract") or "")[:250]
        if ab:
            print(f"     abs: {ab}")

search("Bajura Jones flow distribution manifolds")
search("flow distribution manifold analytical model header")
search("flow distribution parallel channels manifold solution")
search("pressure balance divider manifold flow uniformity")
