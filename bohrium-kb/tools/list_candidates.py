#!/usr/bin/env python3
"""List the 20 new S4 challenges with attempt counts (popularity) + our own submission check."""
import json
import os
import re
import sys
from pathlib import Path

import requests

BASE = "https://play.bohrium.com/api"
CRED = os.path.expanduser("~/.dsh/bohrium_credentials.txt")
LOG = Path(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\_logs")

IDS = [
    "closed-book-four-fermion-three-body-decay-helicity-77fef58e",
    "estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7",
    "faithful-determinant-free-nn-vmc-reproduction-of-t-8d0d6b46",
    "find-a-rigorously-certified-finite-counterexample-45966214",
    "find-an-exact-binary-answer-monogamy-of-entangleme-55ab0d77",
    "find-an-exactly-verifiable-counterexample-to-edge-7141d3ed",
    "ground-state-shell-occupations-and-fbd-universal-3-2e27dc1a",
    "multi-band-electron-phonon-coupling-and-transition-2c57551c",
    "monte-carlo-simulation-of-liquid-chloroform-using-e0469f30",
    "spatial-domain-identification-via-graph-informed-c-35985da3-2",
    "md-simulation-of-repulsive-dislocation-intersectio-32d14849-2",
    "site-projected-magnetic-moments-and-spin-state-ass-9640ab85",
    "dft-crystal-structure-formation-energy-and-charge-730d57d3",
    "convergence-and-efficiency-of-two-phase-and-shock-3b75bda5",
    "muon-edge-reconstruction-sharp-material-boundaries-9839145f",
    "2fe-2s-sparse-ci-variational-energy-minimization-2bf4e3d5",
    "signed-multi-orientation-tdgl-grain-growth-cbc790a3",
    "3d-refractive-index-reconstruction-with-non-paraxi-d8d9babd",
    "spin-3-2-entanglement-power-and-zero-entanglement-f37e43b7",
    "stationary-huggett-equilibrium-with-a-finite-diffe-dda9aff2",
]


def main():
    txt = Path(CRED).read_text(encoding="utf-8")
    tok = re.search(r"api_token\s*=\s*(\S+)", txt).group(1)
    h = {"Authorization": f"Bearer {tok}"}
    rows = []
    for cid in IDS:
        r = requests.get(BASE + f"/challenges/{cid}", headers=h, timeout=60)
        if r.status_code != 200:
            rows.append({"id": cid, "status": r.status_code})
            continue
        c = r.json()
        rows.append({
            "id": cid, "title": c.get("title"), "attempts": c.get("attempts"),
            "difficulty": c.get("difficulty"), "disc": c.get("disc"), "status": c.get("status"),
            "roundSeq": c.get("roundSeq"), "tags": c.get("tags"), "figures": c.get("figures"),
            "created": c.get("createdAt") or c.get("created_at"),
            "maxScore": c.get("maxScore") or c.get("max_score"),
        })
    (LOG / "cand_details.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")

    # our-identity check via attempts list
    ours = []
    for row in rows:
        cid = row["id"]
        try:
            r = requests.get(BASE + f"/challenges/{cid}/attempts", headers=h, params={"limit": 200}, timeout=60)
            if r.status_code != 200:
                row["attempts_status"] = r.status_code
                continue
            body = r.json()
            items = body if isinstance(body, list) else body.get("items") or body.get("attempts") or []
            total = body.get("total") if isinstance(body, dict) else None
            ours_here = [a for a in items if str(a.get("authorId") or a.get("author") or "").startswith("friday")]
            row["attempts_total_api"] = total if total is not None else len(items)
            row["our_attempts"] = [(a.get("id"), a.get("score")) for a in ours_here]
            if ours_here:
                ours.append(cid)
            # sample of field names from first attempt
            if row is rows[0] and items:
                row["_sample_attempt_keys"] = sorted(items[0].keys())
        except Exception as e:
            row["attempts_error"] = str(e)[:120]

    (LOG / "cand_details.json").write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    print("sample attempt keys:", rows[0].get("_sample_attempt_keys"))
    for x in rows:
        print(f"{str(x.get('attempts')):>5} att | d={x.get('difficulty')} | {str(x.get('disc')):<10} | ours={len(x.get('our_attempts') or [])} | {(x.get('title') or x.get('id'))[:75]}")
    print("challenges with OUR attempts:", ours)


if __name__ == "__main__":
    main()
