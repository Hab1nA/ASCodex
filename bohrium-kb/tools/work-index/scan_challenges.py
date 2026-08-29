#!/usr/bin/env python3
"""Scan challenge list, mark done vs new, sort by popularity."""
import json, os

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

base = os.path.dirname(os.path.abspath(__file__))
d = json.load(open(os.path.join(base, 'challenges_full_20260818.json'), encoding='utf-8'))
items = d if isinstance(d, list) else d.get('challenges', d.get('items', []))
items.sort(key=lambda c: -c.get('attempts', 0))
print(f"total: {len(items)}")
print(f"{'ST':<5} {'att':>4} {'diff':<4} {'disc':<22} {'id':<62} title")
for c in items:
    mark = 'DONE' if c['id'] in done else 'NEW '
    print(f"{mark:<5} {c.get('attempts',0):>4} {c.get('difficulty','?'):<4} "
          f"{str(c.get('disc','')):<22} {c['id'][:62]:<62} {c.get('title','')[:48]}")
