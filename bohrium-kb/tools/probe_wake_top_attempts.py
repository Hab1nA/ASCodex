#!/usr/bin/env python3
"""Inspect top-scored attempts on the wake challenge: how did they get the data?"""
import json, os, re, urllib.request, time

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit", "top_attempts")
os.makedirs(OUT, exist_ok=True)


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok, binary=False):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read()
        return data if binary else json.loads(data.decode("utf-8"))


tok = load_token()
d = get("/challenges/wang-2024-pof-dg/attempts", tok)
items = d if isinstance(d, list) else d.get("items", [])
# sort by score desc
items.sort(key=lambda a: -(a.get("score") or 0))
print("fetched:", len(items), "top scores:", [a.get("score") for a in items[:8]])
for a in items[:5]:
    aid = a["id"]
    det = get(f"/attempts/{aid}", tok)
    fn = os.path.join(OUT, f"attempt_{aid}.json")
    json.dump(det, open(fn, "w", encoding="utf-8"), indent=1, ensure_ascii=False)
    keys = {k: det.get(k) for k in ("id", "score", "status", "scriptAvailable", "scriptPath", "bundleAvailable", "bundlePath", "execStatus", "method", "modelTag", "harness")}
    print(json.dumps(keys, ensure_ascii=False))
    el = (det.get("execLog") or "")
    if el:
        print("  execLog head:", el[:400].replace("\n", " | "))
    sd = det.get("scoringDetails")
    if sd:
        print("  scoringDetails:", json.dumps(sd, ensure_ascii=False)[:600])
    time.sleep(0.3)
