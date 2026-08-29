#!/usr/bin/env python3
"""Find how the web UI downloads challenge datasets: fetch SPA html, JS bundles, grep dataset download patterns."""
import urllib.request, re, os, json

OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
os.makedirs(OUT, exist_ok=True)
UA = {"User-Agent": "Mozilla/5.0"}

html = urllib.request.urlopen(urllib.request.Request("https://play.bohrium.com/", headers=UA), timeout=60).read().decode("utf-8")
open(os.path.join(OUT, "spa_index.html"), "w", encoding="utf-8").write(html)
js = re.findall(r'src="([^"]+\.js)"', html)
print("JS bundles:", js)

for j in js:
    url = j if j.startswith("http") else ("https://play.bohrium.com/" + j.lstrip("/"))
    try:
        body = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120).read().decode("utf-8", "replace")
        fn = os.path.join(OUT, "appjs_" + re.sub(r"[^A-Za-z0-9]+", "_", j)[-40:] + ".js")
        open(fn, "w", encoding="utf-8").write(body)
        print(f"saved {fn} ({len(body)} chars)")
        # find dataset-related url patterns
        pats = set(re.findall(r'"[^"{}]{0,60}dataset[^"{}]{0,60}"', body, re.I))
        dl = [p for p in pats if "download" in p.lower() or "file" in p.lower() or "/" in p]
        print("  dataset patterns:", dl[:20])
        api = set(re.findall(r'"/api/[a-zA-Z0-9/_${}.?=&-]+"', body))
        print("  api patterns:", sorted(api)[:40])
    except Exception as e:
        print("FAIL", url, e)
