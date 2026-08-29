---
name: dft-convergence
description: "Systematic convergence testing for density functional theory calculations: sweep plane-wave cutoff, k-point grid, smearing width, and supercell size to ensure computed properties are numerically converged before production runs. Trigger on: 'convergence test', 'converge my DFT parameters', 'k-point convergence', 'cutoff energy sweep', 'check ENCUT', 'ecutwfc convergence', 'smearing test', 'basis set convergence', 'are my parameters converged', 'numerical accuracy check', 'VASP convergence', 'Quantum ESPRESSO convergence'. Also activates when a user is setting up a new DFT calculation and needs to determine safe numerical parameters, or when results look suspicious and parameter convergence should be verified."
---

# /dft-convergence — DFT Convergence Testing

Systematic convergence testing for density functional theory (DFT) calculations. Ensures that computed properties (energies, forces, band gaps, magnetic moments) are converged with respect to all critical numerical parameters before any production run.

## Trigger

User mentions: "convergence test", "DFT convergence", "k-point convergence", "cutoff energy", "encut", "ecutwfc", "smearing convergence", "basis set convergence", "converge parameters", "numerical accuracy check".

## Workflow

### Step 1 — Identify the Target Property

Before converging anything, define **what** you are converging:

| Property Class | Examples | Typical Tolerance |
|----------------|----------|-------------------|
| Total energy | Cohesive energy, formation energy | 1 meV/atom |
| Forces | Relaxation, phonons | 10 meV/Å |
| Electronic | Band gap, DOS, charge density | 0.01 eV |
| Magnetic | Spin moment, MAE | 0.01 μ_B |
| Mechanical | Elastic constants, bulk modulus | 1 GPa |

Different properties converge at different rates. Always state the target property and acceptable tolerance **before** running convergence sweeps.

### Step 2 — Parameter Sweep (One at a Time)

Converge parameters in this order (each held fixed while sweeping the next):

1. **Plane-wave cutoff** (ENCUT / ecutwfc)
   - Start at 1.0× recommended pseudopotential cutoff
   - Sweep in steps of 50 eV (or 5 Ry) up to 2.0× recommended
   - Converged when ΔE < tolerance for 2 consecutive increases

2. **k-point grid** (KPOINTS / K_POINTS)
   - Start at Γ-only, increase density along each reciprocal axis
   - For metals: use Methfessel–Paxton or cold smearing; for insulators: tetrahedron method is acceptable
   - Converged when ΔE < tolerance for 2 consecutive grid refinements
   - Record both Monkhorst-Pack and Γ-centered results — some systems are sensitive to the centering

3. **Smearing width** (SIGMA / degauss)
   - Only relevant for metals or near-metallic systems
   - Sweep: 0.01, 0.05, 0.10, 0.20, 0.50 eV
   - Check that the entropy term (T·S) contributes < 1 meV/atom to the free energy
   - If entropy term is large, reduce smearing or use tetrahedron method

4. **Supercell size** (if computing defects, surfaces, or phonons)
   - Increase cell size until the target property changes by < tolerance
   - Check for spurious image interactions via energy vs. 1/L plots

### Step 3 — Generate Convergence Plots

For each parameter sweep, produce a convergence plot:

```
┌──────────────────────────────────────┐
│ E_cohesive vs ENCUT                  │
│                                      │
│  ●────●────●═══●═══●═══●            │
│                    ▲                 │
│              converged (ΔE < 1 meV)  │
│                                      │
│  ENCUT (eV) →                        │
└──────────────────────────────────────┘
```

Mark the convergence threshold visually. Include a table with raw data:

| ENCUT (eV) | E_total (eV) | ΔE (meV/atom) | Converged? |
|------------|-------------|----------------|------------|
| 300 | -5.234 | — | — |
| 350 | -5.287 | 53.0 | No |
| 400 | -5.291 | 4.0 | No |
| 450 | -5.292 | 1.0 | Yes |
| 500 | -5.292 | 0.0 | Yes |

### Step 4 — Cross-Check

After individual convergence:
1. **Re-test** with all parameters at converged values simultaneously (interactions between parameters can shift convergence)
2. **Compare** with literature values or Materials Project data for the same system
3. **Document** final parameter set with provenance:

```yaml
system: FCC Cu (225, Fm-3m)
xc_functional: PBE
pseudopotential: PAW_PBE Cu 22Jun2005
encut: 450 eV  # converged to 1 meV/atom (Step 2.1)
kpoints: 12×12×12 Γ-centered  # converged to 1 meV/atom (Step 2.2)
smearing: Methfessel-Paxton, σ = 0.10 eV  # T·S = 0.3 meV/atom
converged_property: cohesive energy
tolerance: 1 meV/atom
```

### Step 5 — Report

Write a convergence report including:
- Target property and tolerance
- Convergence plots for each parameter
- Final parameter set
- Comparison with reference data
- Any anomalies (e.g., non-monotonic convergence, strong parameter coupling)

## Common Pitfalls

- **Converging the wrong property.** Converging total energy does not guarantee converged forces. Always converge the property you care about.
- **Forgetting the pseudopotential cutoff floor.** Never go below the recommended cutoff for your pseudopotential.
- **Smearing artifacts in metals.** If ISMEAR/degauss is too large, the free energy ≠ ground state energy. Always check T·S.
- **Symmetry-breaking at low k-points.** Some magnetic or Jahn-Teller systems need odd k-grids to preserve symmetry.
- **Assuming transferability.** Convergence parameters from bulk may not transfer to surfaces, defects, or interfaces.

## Supported Codes

- VASP (INCAR/KPOINTS/POSCAR)
- Quantum ESPRESSO (pw.x input)
- ABINIT
- CASTEP
- Any plane-wave or PAW code with analogous parameters
