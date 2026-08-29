#!/usr/bin/env python3
"""Fetch per-challenge stats for the 17 new S4 challenges."""
import json, os, urllib.request

base = os.path.dirname(os.path.abspath(__file__))
jwt = os.environ.get('FRIDAY_JWT')
assert jwt, "set FRIDAY_JWT"

ids = json.load(open(os.path.join(base, 'hackathon_ids_20260818.json')))
done = set("""thermodynamic-twisting-operator-diagnosis-of-perio-20fdd3fd
construct-a-4-4-ppt-quantum-channel-and-verify-the-1c5c7f97
focused-imaging-and-resolution-characterisation-fr-e287fbca
solving-heterogeneous-agent-models-with-deepham-18a5adeb
flowforge-open-model-selection-flow-v5-a9464888
mp-r-mp-r-ab-uv-split-coann-6924985d
reasoning-gate-separable-covariance-1fe5635b
multi-sample-cnv-detection-from-binned-read-counts-15924b97
mp-r-mp-r-a-uv-portal-a5be12b2
reasoning-gate-gbsde-feynman-kac-e8970329
2fe-2s-sparse-ci-variational-energy-minimization-2bf4e3d5
3d-refractive-index-reconstruction-with-non-paraxi-d8d9babd
muon-edge-reconstruction-sharp-material-boundaries-9839145f
spin-3-2-entanglement-power-and-zero-entanglement-f37e43b7
stationary-huggett-equilibrium-with-a-finite-diffe-dda9aff2""".split())

new_ids = [i for i in ids if i not in done]
print(f"new challenges: {len(new_ids)}")

def get(url):
    req = urllib.request.Request(url, headers={'Authorization': f'Bearer {jwt}'})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read().decode())

out = []
for cid in new_ids:
    try:
        c = get(f"https://play.bohrium.com/api/challenges/{cid}")
        c['_id'] = cid
        out.append(c)
    except Exception as e:
        print(f"ERR {cid}: {e}")

# what stats fields exist?
if out:
    print("fields:", sorted(out[0].keys()))
    print("sample:", json.dumps({k: out[0][k] for k in list(out[0])[:20]}, ensure_ascii=False, default=str)[:800])
with open(os.path.join(base, 'new_s4_challenges.json'), 'w', encoding='utf-8') as f:
    json.dump(out, f, ensure_ascii=False, indent=1, default=str)
