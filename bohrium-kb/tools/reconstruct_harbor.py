#!/usr/bin/env python3
"""Reconstruct harbor formula for R3 Q10 DLPFC using label CSV -> ARI/NMI vs Table S2 target.

Known anchors:
  improved-labels (summary_metrics.csv) -> harbor 0.974083
  original-29108 labels                 -> harbor 0.949288
Then identify which slices contribute the 1.0-0.974 gap (i.e., what full-scorer fixed).
"""
import csv, itertools, json, sys
sys.stdout.reconfigure(encoding="utf-8")

# Table S2 targets (ARI, NMI) from REPORT
T = {
 "151507":(0.505,0.627),"151508":(0.564,0.633),"151509":(0.588,0.664),"151510":(0.569,0.645),
 "151669":(0.561,0.601),"151670":(0.580,0.623),"151671":(0.546,0.596),"151672":(0.621,0.651),
 "151673":(0.571,0.683),"151674":(0.564,0.660),"151675":(0.720,0.759),"151676":(0.627,0.710)}

def load_summary(path):
    rows={}
    for r in csv.DictReader(open(path)):
        rows[r["slice_id"]]=(float(r["ARI"]),float(r["NMI"]))
    return rows

rows = load_summary(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal\bohrium-kb\round1_prep\work\10-spatial\outputs\summary_metrics.csv")

TOL = 0.05

def per_slice_combo(a,n,t):
    c=0.7*a+0.3*n; tc=0.7*t[0]+0.3*t[1]
    d=abs(c-tc)
    if d<=TOL: return 1.0
    if d>=2*TOL: return 0.0
    return 1.0-(d-TOL)/TOL

def per_slice_linband(a,n,t):
    # symmetric: full within TOL, decay to 0 at 2*TOL
    c=0.7*a+0.3*n; tc=0.7*t[0]+0.3*t[1]
    d=abs(c-tc)
    if d<=TOL: return 1.0
    if d>=2*TOL: return 0.0
    return 1.0-(d-TOL)/TOL

def per_slice_arionly(a,n,t):
    # threshold only on ARI within TOL lower
    d=a-t[0]
    if d>= -TOL: return 1.0
    return 1.0-(-TOL-d)/TOL

def harbor(per_func, agg_floor=0.574):
    scores=[]
    ari=[]
    for sid,vals in sorted(rows.items()):
        s=per_func(*vals,T[sid]); scores.append(s); ari.append(vals[0])
    ari.sort(); med=ari[len(ari)//2]
    agg=1.0 if med>=agg_floor else med/agg_floor
    S=sum(scores)/12
    h=0.60*S+0.25*agg+0.15*1.0
    return h,S,scores,med

for name,fn in [("combo_linband",per_slice_linband),("combo_thr",per_slice_combo),("ari_only_lower",per_slice_arionly)]:
    h,S,scores,med=harbor(fn)
    print(f"{name:18s} harbor={h:.6f} S_avg={S:.4f} median_ARI={med:.4f}")
    print("   per-slice:", " ".join(f"{s:.2f}" for s in scores))

# report per-slice deviations (combo)
print("\nPer-slice combo deviations (improved labels):")
for sid in sorted(rows):
    a,n=rows[sid]; t=T[sid]
    c=0.7*a+0.3*n; tc=0.7*t[0]+0.3*t[1]
    print(f"  {sid}: combo={c:.3f} target={tc:.3f} dev={c-tc:+.3f} |d|={abs(c-tc):.3f}")
