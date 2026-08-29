#!/usr/bin/env python3
"""Compare 100/92/85/0-score attempts: bundle availability, resultsJson, scoringDetails."""
import json, os, re, urllib.request, time

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read().decode("utf-8"))


tok = load_token()
items = json.load(open(os.path.join(OUT, "attempts_all.json"), encoding="utf-8"))

picks = {}
for a in items:
    s = round(a.get("score") or 0)
    if s not in picks:
        picks[s] = a
for target in (100, 92, 85, 50, 0):
    a = picks.get(target)
    if not a:
        continue
    aid = a["id"]
    det = get(f"/attempts/{aid}", tok)
    json.dump(det, open(os.path.join(OUT, f"attempt_{aid}_s{target}.json"), "w", encoding="utf-8"), indent=1, ensure_ascii=False)
    print(f"=== score {target} attempt {aid} ({a.get('createdAt')})")
    for k in ("scriptAvailable", "scriptPath", "bundleAvailable", "bundlePath", "bundleStatus", "execStatus", "traceCount", "rawMessagesAvailable"):
        print("  ", k, ":", det.get(k))
    print("  resultsJson:", json.dumps(det.get("resultsJson"), ensure_ascii=False)[:400])
    print("  scoringDetails:", json.dumps(det.get("scoringDetails"), ensure_ascii=False)[:700])
    print("  method:", str(det.get("method"))[:200])
    el = det.get("execLog") or ""
    print("  execLog:", el[:400].replace("\n", " | "))
    time.sleep(0.3)
