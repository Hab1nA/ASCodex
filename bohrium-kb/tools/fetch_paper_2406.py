#!/usr/bin/env python3
"""Fetch arXiv 2406.00695 HTML and extract data-availability + key model sections."""
import urllib.request, re, os

OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"
UA = {"User-Agent": "Mozilla/5.0"}
url = "https://arxiv.org/html/2406.00695v1"
html = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=180).read().decode("utf-8", "replace")
open(os.path.join(OUT, "paper_2406_00695.html"), "w", encoding="utf-8").write(html)
# strip tags to text
text = re.sub(r"<script.*?</script>|<style.*?</style>", " ", html, flags=re.S)
text = re.sub(r"<[^>]+>", " ", text)
text = re.sub(r"&amp;", "&", text)
text = re.sub(r"&lt;", "<", text)
text = re.sub(r"&gt;", ">", text)
text = re.sub(r"&quot;", '"', text)
text = re.sub(r"&#8722;|&minus;", "-", text)
text = re.sub(r"\s+", " ", text)
open(os.path.join(OUT, "paper_2406_00695.txt"), "w", encoding="utf-8").write(text)
print("text chars:", len(text))

for kw in ("Data availability", "data availability", "Supplementary", "supplementary", "GitHub", "github", "zenodo", "Zenodo", "repository", "125", "NREL", "x/D", "CFD", "LES", "RANS"):
    idxs = [m.start() for m in re.finditer(re.escape(kw), text)]
    print(f"\n### '{kw}': {len(idxs)} hits")
    for i in idxs[:4]:
        print("  ...", text[max(0, i - 200):i + 400][:600], "...")
