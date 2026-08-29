#!/usr/bin/env python3
"""Extract the EXACT visible input JSON from the challenge content.
The canary replay depends on exact parameters, so we must materialize it verbatim.
"""
import json, os, re, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')

base = os.path.dirname(os.path.abspath(__file__))
out = json.load(open(os.path.join(base, 'new_s4_challenges.json'), encoding='utf-8'))
c = next(x for x in out if x['_id'] == 'estimate-a-finite-horizon-competing-poisoning-bala-c38a0ad7')
content = c.get('topicContent') or c.get('content') or ''

# Find the "### Visible input" code fence (first ```json ... ``` block after that heading)
idx = content.find('### Visible input')
assert idx >= 0, "Visible input heading not found"
seg = content[idx:]
m = re.search(r'```json\s*(\{.*?\})\s*```', seg, re.DOTALL)
assert m, "Visible input JSON block not found"
raw = m.group(1)
data = json.loads(raw)  # validate it parses
print("parsed OK. cases:", len(data['cases']))
print("bootstrap_replicates:", data['bootstrap_replicates'])
for case in data['cases']:
    print(f"  {case['case_id']}: L={case['lattice_size']} T={case['max_cycles']} "
          f"class={case['execution_class']} surface={case['surface_type']} "
          f"xgrid={len(case['x_a_grid'])} reps={case['replicates']} params={case['parameters']}")

# Save verbatim (preserve exact formatting as found, and also a canonical version)
os.makedirs(os.path.join(base, 'kmc-poi', 'inputs'), exist_ok=True)
with open(os.path.join(base, 'kmc-poi', 'inputs', 'cases.json'), 'w', encoding='utf-8') as f:
    f.write(raw)
# also canonical
with open(os.path.join(base, 'kmc-poi', 'inputs', 'cases_canonical.json'), 'w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)
print("\nWrote kmc-poi/inputs/cases.json (verbatim) and cases_canonical.json")
import hashlib
print("sha256(verbatim):", hashlib.sha256(raw.encode('utf-8')).hexdigest())
