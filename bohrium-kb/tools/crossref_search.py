#!/usr/bin/env python3
"""Crossref search for Bajura-Jones manifold papers."""
import json
import sys
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")

def search(q, rows=6):
    url = "https://api.crossref.org/works?query.bibliographic=" + urllib.parse.quote(q) + \
          f"&rows={rows}&select=title,author,issued,DOI,container-title"
    try:
        with urllib.request.urlopen(url, timeout=60) as r:
            d = json.load(r)
    except Exception as e:
        print("ERR", q, e)
        return
    print("===", q)
    for p in d["message"]["items"]:
        auth = ", ".join(a.get("family", "") for a in p.get("author", [])[:3])
        yr = p.get("issued", {}).get("date-parts", [[None]])[0][0]
        print(f"  [{yr}] {p.get('title', [''])[0][:95]}")
        print(f"     {auth} | DOI {p.get('DOI')} | {p.get('container-title', [''])[0][:50]}")

search("Bajura Jones Flow Distribution Manifolds")
search("flow distribution manifold analytical model")
search("flow distribution in manifolds Wang")
search("pressure balanced manifold flow distribution")
