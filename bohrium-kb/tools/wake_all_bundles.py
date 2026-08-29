#!/usr/bin/env python3
"""Check bundle status for ALL 100-score attempts; download any live bundle."""
import json, os, re, urllib.request, time, zipfile, io

BASE = "https://play.bohrium.com/api"
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "wake-dg-fit", "bundles")
os.makedirs(OUT, exist_ok=True)


def load_token():
    txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
    return re.search(r"api_token\s*=\s*(\S+)", txt).group(1)


tok = load_token()
items = json.load(open(os.path.join(WS, "work", "wake-dg-fit", "attempts_all.json"), encoding="utf-8"))
hundred = [a for a in items if round(a.get("score") or 0) >= 100]
print("100+ attempts:", len(hundred))

for a in hundred:
    aid = a["id"]
    try:
        req = urllib.request.Request(BASE + f"/attempts/{aid}/bundle/status", headers={"Authorization": "Bearer " + tok})
        st = json.loads(urllib.request.urlopen(req, timeout=60).read().decode("utf-8"))
        bs = st.get("bundleStatus")
        if bs == "ready" and st.get("bundlePath"):
            print(f"  {aid} ready {st.get('bundlePath')} scorecard={st.get('scorecard')}")
            # try download
            try:
                req2 = urllib.request.Request(BASE + f"/attempts/{aid}/bundle", headers={"Authorization": "Bearer " + tok})
                data = urllib.request.urlopen(req2, timeout=120).read()
                fn = os.path.join(OUT, f"attempt_{aid}_arm.zip")
                open(fn, "wb").write(data)
                z = zipfile.ZipFile(io.BytesIO(data))
                names = z.namelist()
                hit = [n for n in names if "field" in n.lower() or n.lower().endswith(".txt")]
                print(f"    DOWNLOADED {len(data)}B entries={len(names)} data-like={hit[:6]}")
                for n in hit[:3]:
                    z.extract(n, os.path.join(OUT, f"x_{aid}"))
                    print(f"    extracted {n}")
            except Exception as e:
                print("    download FAIL:", str(e)[:80])
        else:
            pass
        time.sleep(0.2)
    except Exception as e:
        print(f"  {aid} status FAIL: {str(e)[:60]}")
print("done")
