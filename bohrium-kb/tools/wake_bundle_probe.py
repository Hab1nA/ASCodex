#!/usr/bin/env python3
"""Try to download attempt bundles (100/92/85-score) and list contents looking for field.txt."""
import json, os, re, urllib.request, zipfile, io

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit", "bundles")
os.makedirs(OUT, exist_ok=True)


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


tok = load_token()
items = json.load(open(os.path.join(WS, "work", "wake-dg-fit", "attempts_all.json"), encoding="utf-8"))
# pick recent high-score + mid-score attempts
picks = []
seen = set()
for a in items:
    s = round(a.get("score") or 0)
    if a.get("createdAt", "") > "2026-05-18" and s in (100, 92, 85, 50) and a["id"] not in seen:
        seen.add(a["id"])
        picks.append((s, a["id"]))
picks.sort(reverse=True)
print("picks:", picks)

for s, aid in picks[:6]:
    for ep in (f"/attempts/{aid}/bundle", f"/attempts/{aid}/script", f"/attempts/{aid}/download"):
        try:
            req = urllib.request.Request(BASE + ep, headers={"Authorization": "Bearer " + tok})
            r = urllib.request.urlopen(req, timeout=120)
            data = r.read()
            ct = r.headers.get("Content-Type", "")
            fn = os.path.join(OUT, f"{aid}_{ep.strip('/').replace('/', '_')}_{os.path.splitext(r.headers.get('Content-Disposition','').split('filename=')[-1].strip('"') or ('zip' if 'zip' in ct else 'bin'))}")
            open(fn, "wb").write(data)
            print(f"OK {aid} {ep} -> {len(data)}B {ct} -> {os.path.basename(fn)}")
            if "zip" in ct or data[:2] == b"PK":
                z = zipfile.ZipFile(io.BytesIO(data))
                names = z.namelist()
                print("   entries:", len(names))
                for n in names:
                    if any(k in n.lower() for k in ("field", "data", "txt", "csv", "npz")):
                        print("   DATA-LIKE:", n)
                for n in names[:25]:
                    print("    ", n)
            break
        except Exception as e:
            print(f"FAIL {aid} {ep} -> {str(e)[:90]}")
