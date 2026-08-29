#!/usr/bin/env python3
"""Fetch ALL attempts for the wake challenge (limit=100 pages), verify ours=0, inspect top scorer script."""
import json, os, re, urllib.request, time, collections

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok, binary=False):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read()
        return data if binary else json.loads(data.decode("utf-8"))


tok = load_token()
items = []
for p in (1, 2, 3):
    d = get(f"/challenges/wang-2024-pof-dg/attempts?limit=100&page={p}", tok)
    chunk = d["attempts"] if isinstance(d, dict) else d
    items.extend(chunk)
    if len(chunk) < 100:
        break
json.dump(items, open(os.path.join(OUT, "attempts_all.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
print("total attempts:", len(items))
ours = [a for a in items if a.get("operatorId") == "1179613" or str(a.get("authorId", "")).startswith("friday") or str(a.get("author_name") or "").startswith("friday")]
print("ours:", len(ours))
for o in ours:
    print("  OURS:", o.get("id"), o.get("score"), o.get("createdAt"))
print("score dist:", dict(collections.Counter(round(a.get("score") or 0) for a in items)))
print("status dist:", dict(collections.Counter(str(a.get("status")) for a in items)))
dates = sorted(a.get("createdAt", "") for a in items)
print("oldest:", dates[0], " newest:", dates[-1])
recent = [a for a in items if a.get("createdAt", "") > "2026-08-01"]
print("attempts since 2026-08-01:", len(recent))
items.sort(key=lambda a: (-(a.get("score") or 0), a.get("createdAt", "")))
# inspect the newest 100-scorer's bundle/script
for a in items:
    if (a.get("score") or 0) >= 100:
        aid = a["id"]
        det = get(f"/attempts/{aid}", tok)
        json.dump(det, open(os.path.join(OUT, f"attempt_{aid}.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
        print("\ntop attempt", aid, a.get("createdAt"))
        print("  scriptAvailable:", det.get("scriptAvailable"), "scriptPath:", det.get("scriptPath"))
        print("  bundleAvailable:", det.get("bundleAvailable"), "bundlePath:", det.get("bundlePath"))
        print("  execStatus:", det.get("execStatus"))
        el = det.get("execLog") or ""
        print("  execLog head:", el[:500].replace("\n", " | "))
        print("  scoringDetails:", json.dumps(det.get("scoringDetails"), ensure_ascii=False)[:500])
        print("  resultsJson:", json.dumps(det.get("resultsJson"), ensure_ascii=False)[:400])
        break
