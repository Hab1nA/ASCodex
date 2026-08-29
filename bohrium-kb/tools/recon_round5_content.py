#!/usr/bin/env python3
"""Fetch /content for top free candidates; save full markdown; print head + scoring-contract sections."""
import json, os, re, urllib.request, time

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
os.makedirs(OUT, exist_ok=True)

IDS = [
    "convergence-and-efficiency-of-two-phase-and-shock-3b75bda5",
    "estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad",
    "wang-2024-pof-dg",
    "ground-state-shell-occupations-and-fbd-universal-3-2e27dc1",
    "s3-03",
    "multi-band-electron-phonon-coupling-and-transition-2c57551",
    "zhang-2018-prl-deepmd",
    "monte-carlo-simulation-of-liquid-chloroform-using-e0469f30",
]


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


tok = load_token()
for cid in IDS:
    try:
        req = urllib.request.Request(BASE + f"/challenges/{cid}/content", headers={"Authorization": "Bearer " + tok})
        with urllib.request.urlopen(req, timeout=90) as r:
            txt = r.read().decode("utf-8")
        fn = os.path.join(OUT, f"content_{cid}.md")
        open(fn, "w", encoding="utf-8").write(txt)
        print(f"=== {cid} ({len(txt)} chars) -> {os.path.basename(fn)}")
        # head
        print("HEAD:", txt[:400].replace("\n", " | "))
        # scoring-ish sections: find headings containing score/acceptance/verifier etc.
        for m in re.finditer(r"(?im)^#{1,4}.*(?:score|scoring|verif|accept|grade|rubric|tolerance|submit|deliver|output).*", txt):

            start = m.start()
            # find next heading at same or higher level
            nxt = re.search(r"(?m)^#{1,4} ", txt[start + 10:])
            end = start + 10 + nxt.start() if nxt else min(start + 3000, len(txt))
            seg = txt[start:end]
            print(f"  [sec] {m.group(0)[:90]}")
            print("       " + seg[:1200].replace("\n", " "))
        time.sleep(0.3)
    except Exception as e:
        print(f"=== {cid} FAIL {e}")
