#!/usr/bin/env python3
"""Scan S4 hackathon challenges: popularity + done-status."""
import json, os, urllib.request

base = os.path.dirname(os.path.abspath(__file__))

# 1) hackathon challenge ids
h = json.load(urllib.request.urlopen("https://play.bohrium.com/api/hackathon/current", timeout=60))
ids = h["challengeIds"]
print(f"hackathon challenges: {len(ids)}")
json.dump(ids, open(os.path.join(base, 'hackathon_ids_20260818.json'), 'w'), indent=1)

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

# 2) full challenge list -> stats by id
d = json.load(open(os.path.join(base, 'challenges_full_20260818.json'), encoding='utf-8'))
items = d if isinstance(d, list) else d.get('challenges', d.get('items', []))
byid = {c['id']: c for c in items}

rows = []
for cid in ids:
    c = byid.get(cid, {})
    rows.append({
        'id': cid,
        'title': c.get('title', '?'),
        'disc': c.get('disc', '?'),
        'diff': c.get('difficulty', '?'),
        'attempts': c.get('attempts', 0),
        'stars': c.get('stars', 0),
        'status': c.get('status', '?'),
        'done': cid in done,
    })
rows.sort(key=lambda r: (-r['attempts'], r['id']))
print(f"{'ST':<5} {'att':>4} {'star':>4} {'diff':<4} {'disc':<22} id")
for r in rows:
    mark = 'DONE' if r['done'] else 'NEW '
    print(f"{mark:<5} {r['attempts']:>4} {r['stars']:>4} {str(r['diff']):<4} {r['disc']:<22} {r['id']}")
