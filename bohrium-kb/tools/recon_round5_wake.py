#!/usr/bin/env python3
"""Target challenge deep recon: detail + datasets + full attempts pagination (ours check)."""
import json, os, re, urllib.request, time

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")
os.makedirs(OUT, exist_ok=True)
CID = "wang-2024-pof-dg"


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok, binary=False):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read()
        return data if binary else json.loads(data.decode("utf-8"))


tok = load_token()

# 1) challenge detail
detail = get(f"/challenges/{CID}", tok)
json.dump(detail, open(os.path.join(OUT, "challenge_detail.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print("detail keys:", sorted(detail.keys()))
print("datasets field:", json.dumps(detail.get("datasets"), ensure_ascii=False)[:800])
for k in ("dataset", "resources", "files", "assets"):
    if k in detail and detail[k]:
        print(f"{k}:", json.dumps(detail[k], ensure_ascii=False)[:800])

# 2) datasets endpoint variants
for p in (f"/challenges/{CID}/datasets", f"/challenges/{CID}/files", f"/datasets?challengeId={CID}"):
    try:
        d = get(p, tok)
        fn = os.path.join(OUT, "probe_ds_" + p.split("/")[-1].replace("?", "_").replace("=", "") + ".json")
        json.dump(d, open(fn, "w", encoding="utf-8"), indent=1)
        print("OK", p, "->", json.dumps(d, ensure_ascii=False)[:400])
    except Exception as e:
        print("FAIL", p, "->", e)

# 3) full attempts pagination
total = 0
ours = []
p = 1
while True:
    d = get(f"/challenges/{CID}/attempts?page={p}&per_page=100", tok)
    items = d if isinstance(d, list) else (d.get("items") or d.get("attempts") or [])
    if isinstance(d, dict) and "items" not in d and "attempts" not in d:
        # maybe {data: {items: ...}}
        for k in ("data", "results"):
            if k in d and isinstance(d[k], dict) and "items" in d[k]:
                items = d[k]["items"]
                break
    if not isinstance(items, list):
        print("unexpected attempts shape:", json.dumps(d, ensure_ascii=False)[:300])
        break
    total += len(items)
    for a in items:
        if a.get("operatorId") == "1179613" or str(a.get("authorId", "")).startswith("friday") or str(a.get("authorName", "")).startswith("friday"):
            ours.append({k: a.get(k) for k in ("id", "authorId", "score", "status", "createdAt")})
    if isinstance(d, dict) and not d.get("has_more", len(items) == 100):
        break
    if len(items) < 100:
        break
    p += 1
    if p > 20:
        break
print(f"\nattempts total fetched: {total}, ours: {len(ours)}")
for o in ours:
    print("  OURS:", o)
if items:
    print("sample attempt keys:", sorted(items[0].keys()))
    print("sample:", json.dumps({k: items[0].get(k) for k in ("id", "authorId", "authorName", "operatorId", "status", "score", "harbor", "trace", "createdAt")}, ensure_ascii=False))
    # distribution of scores
    import collections
    print("status dist:", dict(collections.Counter(str(a.get("status")) for a in items)))
