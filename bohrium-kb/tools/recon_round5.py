#!/usr/bin/env python3
"""Round-5 recon: list platform challenges, rank by popularity, mark challenges we already have attempts on.

Outputs:
  - work/round5-recon/challenges_raw.json   (raw challenge list)
  - work/round5-recon/attempts_cache.json   (per-challenge attempt lists, for later filtering)
  - stdout: ranked table  slug | attempts | ours | title
"""
import json, os, re, sys, time, urllib.request, urllib.error

BASE = os.environ.get("BOHRIUM_BASE", "https://play.bohrium.com/api")
WS = r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal"
OUT = os.path.join(WS, "work", "round5-recon")
os.makedirs(OUT, exist_ok=True)

# ---- blacklist (slug prefixes we already fought, from SELECTION_BLACKLIST.md + work/ dirs) ----
BLACKLIST_PREFIX = [
    "focused-imaging-and-resolution-characterisation",
    "thermodynamic-twisting-operator-diagnosis",
    "construct-a-4-4-ppt-quantum-channel",
    "reasoning-gate-gbsde-feynman-kac",
    "reasoning-gate-separable-covariance",
    "multi-sample-cnv-detection",
    "solving-heterogeneous-agent-models-with-deepham",
    "flowforge-open-model-selection-flow",
    "mp-r-mp-r-ab-uv-split-coann",
    "mp-r-mp-r-a-uv-portal",
    "lax-wendroff",
    "2fe-2s-sparse-ci-variational-energy-minimization",
    "3d-refractive-index-reconstruction",
    "muon-edge-reconstruction",
    "stationary-huggett-equilibrium",
    "spin-3-2-entanglement-power",
    "euler-number-approximation",
    "unsteady-cascade-transfer-functions",
]
OUR_OPERATOR = "1179613"


def load_token(path=None):
    path = path or os.path.expanduser(r"~\.dsh\bohrium_credentials.txt")
    txt = open(path, encoding="utf-8").read()
    m = re.search(r"api_token\s*=\s*(\S+)", txt)
    if not m:
        raise SystemExit(f"no api_token in {path}")
    return m.group(1)


def get(path, tok):
    req = urllib.request.Request(BASE + path, headers={"Authorization": "Bearer " + tok, "Accept": "application/json"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=90) as r:
                return json.loads(r.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            body = e.read().decode("utf-8", "replace")[:300]
            raise SystemExit(f"HTTP {e.code} on {path}: {body}")
        except Exception as e:
            if attempt == 2:
                raise
            time.sleep(2)


def main():
    tok = load_token()
    ch = get("/challenges", tok)
    if isinstance(ch, dict):
        for k in ("challenges", "items", "data", "results"):
            if k in ch:
                ch = ch[k]
                break
    if not isinstance(ch, list):
        raise SystemExit(f"unexpected /challenges shape: {type(ch)} {str(ch)[:400]}")
    json.dump(ch, open(os.path.join(OUT, "challenges_raw.json"), "w"), indent=1)
    print(f"total challenges: {len(ch)}")
    print("sample keys:", sorted(ch[0].keys()) if ch else None)
    if ch:
        print("sample item:", json.dumps(ch[0], ensure_ascii=False)[:600])

    rows = []
    cache = {}
    for c in ch:
        cid = c.get("id") or c.get("slug") or c.get("challengeId")
        title = (c.get("title") or c.get("name") or "")[:70]
        slug = c.get("slug") or ""
        try:
            att = get(f"/challenges/{cid}/attempts", tok)
            if isinstance(att, dict):
                for k in ("attempts", "items", "data", "results"):
                    if k in att:
                        att = att[k]
                    break
            att = att if isinstance(att, list) else []
        except SystemExit as e:
            print(f"  ! attempts failed for {cid}: {e}")
            att = []
        cache[str(cid)] = att
        ours = sum(1 for a in att if (a.get("operatorId") == OUR_OPERATOR
                                      or str(a.get("authorId", "")).startswith("friday")
                                      or str(a.get("authorName", "")).startswith("friday")))
        bl = any(p in str(slug) or p in str(cid) for p in BLACKLIST_PREFIX)
        rows.append({"id": cid, "slug": slug, "title": title, "n_att": len(att), "ours": ours, "blacklisted": bl,
                     "meta": {k: c.get(k) for k in ("difficulty", "discipline", "tags", "createdAt", "popularity", "status", "publishedAt")}})
    json.dump(cache, open(os.path.join(OUT, "attempts_cache.json"), "w"))
    rows.sort(key=lambda r: (r["ours"] > 0, r["blacklisted"], -r["n_att"]))
    print()
    for r in rows:
        mark = "OURS " if r["ours"] else ("BL    " if r["blacklisted"] else "      ")
        print(f"{mark} att={r['n_att']:<4} ours={r['ours']:<2} {str(r['id'])[:64]} | {r['title']}")


if __name__ == "__main__":
    main()
