import urllib.request, os, sys
base="https://ndownloader.figshare.com/files/"
items = {
 "151508":"39055589","151509":"39055586","151510":"39055583","151669":"39055580",
 "151670":"39055577","151671":"39055574","151672":"39055571","151673":"39055568",
 "151674":"39055565","151675":"39055562","151676":"39055559"
}
outdir = sys.argv[1]
missing_ok = True
for sid,fid in items.items():
    dest = os.path.join(outdir, f"{sid}.h5ad")
    if os.path.exists(dest) and os.path.getsize(dest) > 90_000_000:
        print(f"{sid}: already present ({os.path.getsize(dest)} B)", flush=True); continue
    url = base + fid
    print(f"{sid}: downloading {url} ...", flush=True)
    req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=600) as r, open(dest,"wb") as f:
        import shutil; shutil.copyfileobj(r, f)
    print(f"{sid}: done {os.path.getsize(dest)} B", flush=True)
print("ALL DONE", flush=True)
