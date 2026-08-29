#!/usr/bin/env python3
import json, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8', errors='replace')
d = json.load(open('work/protocol.json', encoding='utf-8'))
mod = d.get('modalities', {})
print("all modalities:", mod.get('all'))
print("\nlayout (required files per modality):")
for m, spec in mod.get('layout', {}).items():
    print(f"  {m}: required={spec.get('required')} paths={spec.get('paths')}")
