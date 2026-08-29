#!/usr/bin/env python3
"""Check AIP article page for supplemental material; search arXiv comments for data links."""
import urllib.request, re, os, json

UA = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"}
OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"

# 1) AIP article page
try:
    url = "https://pubs.aip.org/aip/pof/article/36/10/105110/3314968/Discovering-an-interpretable-mathematical"
    html = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=90).read().decode("utf-8", "replace")
    open(os.path.join(OUT, "aip_page.html"), "w", encoding="utf-8").write(html)
    print("AIP page:", len(html))
    for kw in ("Supplemental", "supplement", "Data Availability", "data availability", "10.1063/5.0221611.s1", ".s1", "media"):
        idxs = [m.start() for m in re.finditer(re.escape(kw), html)]
        print(f"  '{kw}': {len(idxs)} hits")
        for i in idxs[:3]:
            seg = html[max(0, i - 150):i + 250].replace("\n", " ")
            print("    ...", re.sub(r"\s+", " ", seg)[:380])
except Exception as e:
    print("AIP page FAIL:", e)

# 2) arXiv abstract page comments/links
try:
    url = "https://arxiv.org/abs/2406.00695"
    html = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=90).read().decode("utf-8", "replace")
    open(os.path.join(OUT, "arxiv_abs.html"), "w", encoding="utf-8").write(html)
    m = re.search(r'Comments:.*?</td>', html, re.S)
    print("\narXiv comments:", re.sub(r"<[^>]+>\s*", " ", m.group(0))[:300] if m else "none")
    for link in set(re.findall(r'href="([^"]*(?:github|zenodo|figshare|osf|data)[^"]*)"', html, re.I)):
        print("  link:", link)
except Exception as e:
    print("arXiv abs FAIL:", e)
