#!/usr/bin/env python3
import urllib.request, re, os

UA = {"User-Agent": "Mozilla/5.0"}
OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
for name in ("catalog.js", "detail.js"):
    url = f"https://play.bohrium.com/js/pages/{name}?v=202608160044"
    body = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120).read().decode("utf-8", "replace")
    fn = os.path.join(OUT, "pagejs_" + name)
    open(fn, "w", encoding="utf-8").write(body)
    print(f"== {name} ({len(body)} chars)")
    for m in re.finditer(r"url|download|blob|field", body, re.I):
        s = max(0, m.start() - 150)
        seg = body[s:m.end() + 250].replace("\n", " ")
        if "dataset" in seg.lower() or "download" in seg.lower() or "url" in m.group(0).lower():
            print("  CTX:", seg[:420])
            print("  ---")
