# Thermodynamic twisting operator for periodic spin chains

## Task

Compute the thermodynamic-limit squared twisting expectation for the periodic antiferromagnetic Heisenberg chains with on-site spin $S=1/2$ and $S=1$:

$$
H_S=\sum_{j=1}^{L}\boldsymbol S_j\cdot\boldsymbol S_{j+1},
\qquad \boldsymbol S_{L+1}=\boldsymbol S_1.
$$

Use coupling one and the zero-magnetization or singlet ground-state sector. Define

$$
U_2(L)=\left\langle\exp\left(\frac{4\pi i}{L}\sum_{j=1}^{L}j\hat S_j^z\right)\right\rangle
$$

and the full-energy variance per site

$$
q=\frac{\langle H^2\rangle-\langle H\rangle^2}{L}.
$$

Use the bundled paper to determine the appropriate thermodynamic extrapolation for each spin system and state the resulting gapped or gapless conclusion.

## Hard requirements

For each spin system:

- use PBCs and include at least the four even sizes $L=32,64,128,256$;
- submit exactly one selected row per size: the largest completed requested bond dimension at that size;
- record `L`, `requested_D`, `actual_D`, total `energy`, `q`, and `U2`; `truncation_error` is optional;
- use only rows with $q<5\times10^{-5}$ in the extrapolation;
- retain at least three valid sizes among the four required sizes;
- declare the finite-size series family and consecutive correction orders used;
- if $n$ sizes are fitted, use at most $n-1$ fitted parameters, including the thermodynamic intercept;
- submit the fitted coefficient list with the intercept first, RMS residual, used sizes, and thermodynamic value;
- use `phase_label: "gapless"` or `phase_label: "gapped_consistent"`;
- keep finite-size and thermodynamic $U_2$ within the unitary-expectation bound, up to numerical tolerance.

The accepted series-family labels are `inverse_size_series` and `inverse_log_series`. The task does not specify which family belongs to which spin system; infer that from the paper and the computed data.

## Required outputs

Write all three files to `/app/outputs`:

- `u2_spin_half.json`;
- `u2_spin_one.json`;
- `fit_u2.py`.

Use the bundled JSON templates. `fit_u2.py` must be valid Python and define `fit_spin_half(samples)` and `fit_spin_one(samples)`. Each JSON must include the selected data, fit metadata, coefficients, `thermodynamic_U2`, `phase_label`, and a short twisting-criterion justification.

The verifier validates the data contract, compares the required finite-size data with withheld references, and independently reconstructs the declared fit. A system that fails these checks receives no credit. Each valid spin system contributes $0.5$; absolute error at most $0.05$ receives full credit, and credit decreases linearly to zero at error $0.10$.
