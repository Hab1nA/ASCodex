#!/usr/bin/env python3
"""Hunt for field.txt: static paths, app.js dataset tab logic, discuss zone."""
import urllib.request, re, os, json

UA = {"User-Agent": "Mozilla/5.0"}
OUT = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\work\wake-dg-fit"

# 1) static path guesses
cands = [
    "/files/wang2024-wake-field/field.txt",
    "/data/wang2024-wake-field/field.txt",
    "/datasets/wang2024-wake-field/field.txt",
    "/files/datasets/wang2024-wake-field/field.txt",
    "/static/wang2024-wake-field/field.txt",
    "/uploads/wang2024-wake-field/field.txt",
    "/files/field.txt",
]
for p in cands:
    try:
        req = urllib.request.Request("https://play.bohrium.com" + p, headers=UA, method="HEAD")
        r = urllib.request.urlopen(req, timeout=30)
        ct = r.headers.get("Content-Type", "")
        ln = r.headers.get("Content-Length", "?")
        print("HEAD OK", p, r.status, ct, ln)
        if "html" not in ct:
            body = urllib.request.urlopen(urllib.request.Request("https://play.bohrium.com" + p, headers=UA), timeout=60).read()
            open(os.path.join(OUT, "field_candidate.txt"), "wb").write(body)
            print("  saved field_candidate.txt", len(body), body[:120])
            break
    except Exception as e:
        print("HEAD FAIL", p, str(e)[:80])

# 2) app.js: how does the challenge Datasets tab render / download?
body = open(os.path.join(OUT, "appjs_main.js"), encoding="utf-8").read()
idxs = [m.start() for m in re.finditer(r"datasets", body, re.I)]
print("\ndatasets occurrences in app.js:", len(idxs))
seen = set()
for i in idxs:
    seg = body[max(0, i - 200):i + 300].replace("\n", " ")
    key = seg[:80]
    if key in seen:
        continue
    seen.add(key)
    if any(w in seg.lower() for w in ("download", "url", "file", "tab", "challenge")):
        print("CTX:", seg[:450])
        print("---")
    if len(seen) > 25:
        break

# 3) discuss zone for the challenge
def get(p, tok):
    req = urllib.request.Request("https://play.bohrium.com/api" + p, headers={"Authorization": "Bearer " + tok, "User-Agent": UA["User-Agent"]})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode("utf-8"))

txt = open(os.path.expanduser(r"~\.dsh\bohrium_credentials.txt"), encoding="utf-8").read()
tok = re.search(r"api_token\s*=\s*(\S+)", txt).group(1)
for p in ("/discuss/topics?challengeId=wang-2024-pof-dg", "/topics?challengeId=wang-2024-pof-dg", "/discuss?challengeId=wang-2024-pof-dg"):
    try:
        d = get(p, tok)
        s = json.dumps(d, ensure_ascii=False)
        print("\nOK", p, "->", s[:600])
    except Exception as e:
        print("\nFAIL", p, str(e)[:100])
