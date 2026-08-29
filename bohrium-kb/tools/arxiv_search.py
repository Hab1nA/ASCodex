#!/usr/bin/env python3
"""Search arXiv API for key papers (web_search backend is down)."""
import sys
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

sys.stdout.reconfigure(encoding="utf-8")
NS = {"a": "http://www.w3.org/2005/Atom"}

def search(query, max_results=6):
    q = urllib.parse.quote(query)
    url = f"http://export.arxiv.org/api/query?search_query={q}&max_results={max_results}&sortBy=relevance"
    with urllib.request.urlopen(url, timeout=60) as r:
        data = r.read().decode("utf-8")
    root = ET.fromstring(data)
    out = []
    for e in root.findall("a:entry", NS):
        title = " ".join(e.find("a:title", NS).text.split())
        aid = e.find("a:id", NS).text
        pub = e.find("a:published", NS).text[:10]
        authors = ", ".join(a.find("a:name", NS).text for a in e.findall("a:author", NS)[:4])
        summ = " ".join(e.find("a:summary", NS).text.split())[:260]
        out.append((title, aid, pub, authors, summ))
    return out

for q in sys.argv[1:]:
    print(f"\n===== QUERY: {q}")
    try:
        for t in search(q):
            print(f"* {t[0]}\n  {t[3]} | {t[2]} | {t[1]}\n  {t[4]}\n")
    except Exception as ex:
        print("  FAILED:", ex)
