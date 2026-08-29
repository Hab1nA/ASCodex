#!/usr/bin/env python3
"""Scan the discuss zone for the wake challenge: data links, setter replies."""
import json, os, re, urllib.request

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit")


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


def get(path, tok):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))


tok = load_token()

# discover discuss endpoints
for p in ("/discuss", "/discuss/topics", "/topics", "/discussions"):
    try:
        d = get(p, tok)
        s = json.dumps(d, ensure_ascii=False)
        print("OK", p, "->", len(s), "chars; head:", s[:300])
    except Exception as e:
        print("FAIL", p, str(e)[:90])

# try challenge-filtered variants
for p in ("/discuss/topics?challenge=wang-2024-pof-dg", "/discuss/topics?challengeId=wang-2024-pof-dg",
          "/discuss/search?q=wang-2024-pof-dg", "/discuss/search?q=wake"):
    try:
        d = get(p, tok)
        s = json.dumps(d, ensure_ascii=False)
        print("OK", p, "->", s[:400])
    except Exception as e:
        print("FAIL", p, str(e)[:90])
