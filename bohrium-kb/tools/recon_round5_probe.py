#!/usr/bin/env python3
"""Probe alternative endpoints for the real challenge list."""
import json, os, re, urllib.request

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
os.makedirs(OUT, exist_ok=True)


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok, "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))


tok = load_token()
paths = ["/agent/work", "/agent/work?limit=100", "/challenges?limit=200", "/challenges?round=5", "/rounds", "/leaderboard"]
for p in paths:
    try:
        d = get(p, tok)
        fn = os.path.join(OUT, "probe_" + p.strip("/").replace("?", "_").replace("=", "").replace("&", "_").replace(" ", "_") + ".json")
        json.dump(d, open(fn, "w"), indent=1)
        if isinstance(d, list):
            print(f"OK {p} -> list[{len(d)}] sample: {json.dumps(d[0], ensure_ascii=False)[:300] if d else 'empty'}")
        else:
            print(f"OK {p} -> dict keys: {sorted(d.keys())[:20]} | {json.dumps(d, ensure_ascii=False)[:300]}")
    except Exception as e:
        print(f"FAIL {p} -> {e}")
