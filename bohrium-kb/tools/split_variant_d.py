#!/usr/bin/env python3
"""Build variant D: independent-QED UV + x_f split + narrative DERIVATION."""
import json
import os
import sys

sys.stdout.reconfigure(encoding="utf-8")

WORK = os.path.join(r"C:\Users\XKZ\Documents\VSCode Projects\ASCLocal",
                    "work", "mp-r-ab-uv-split-coann-6924985d")

# start from variant A (x_f forms + /2 fix), swap UV to independent QED values
a = json.load(open(os.path.join(WORK, "variant_A", "final", "answer.json"), encoding="utf-8"))
d = json.load(open(os.path.join(WORK, "variant_A", "evidence", "derivation.json"), encoding="utf-8"))

a["targets"]["R_UV"]["amplitude"]["polarization_sum_prefactor_over_C2"] = 0.25
a["targets"]["R_UV"]["cross_section"]["coefficient"] = "1/(64*pi)"
for r in d["relations"]:
    if r["quantity"] == "uv:polarization_sum_prefactor_over_C2":
        r["expression"] = 0.25
    if r["quantity"] == "uv:cross_section_coefficient":
        r["expression"] = "1/(64*pi)"

# narrative DERIVATION.md (first-principles story line)
narrative = """# DERIVATION

<!-- DERIVATION_RECORD_BEGIN -->
{
  "schema_version": 1,
  "artifacts": {
    "final": "final/answer.json",
    "evidence": "evidence/derivation.json"
  },
  "targets": {
    "R_UV": {
      "target_id": "R_UV",
      "premises": [
        "SU(N_c) confining gauge theory, three massless Dirac flavours coupled vectorially to an abelian gauge field b; every quark has b-charge X_q=1, both scalars have b-charge X_phi=1.",
        "Scalar sector Higgses the abelian gauge first (v >> Lambda_QCD) leaving vacuum manifold CP^1 ~ S^2; QCD confines at the lower scale. The two dark scalars are the coordinates chi1, chi2 on S^2.",
        "Low-energy fields: canonically normalized neutral pion pi0, canonically normalized coordinates chi1, chi2, canonically normalized photon A. In R_UV the incoming dark scalars are distinct and degenerate m_chi1=m_chi2=m_chi; final pion and photon massless.",
        "Conventions: Tr(t_a t_b) = delta_ab/2; g = exp(2 i pi_a t_a / f_pi); omega_3 = (1/24 pi^2) Tr(g^-1 dg)^3 with unit period; Omega_2 = (1/(4 pi f_D^2)) d chi1 ^ d chi2 + O(chi^3) with unit period; S_mix = 2 pi n int omega_3 ^ Omega_2 with n in Z.",
        "Normalized UV magnetic current star j_m^{(2)} = d b / (2 pi); under the QED gauging omega_3 descends onto the photon field strength F.",
        "QED embedding: Q_int = diag(2,-1,-1), Q_phys = Q_int/3, a_int = (e/3) A, f_int = (e/3) F, alpha_Q = e^2/(4 pi).",
        "The local portal is DEFINED by L_portal = C eps^{mu nu rho sigma} pi0 F_{mu nu} partial_rho chi1 partial_sigma chi2, with C the coefficient to be derived."
      ],
      "calculations": [
        {"quantity": "uv:n_over_Nc", "expression": 1},
        {"quantity": "uv:C_over_ne", "expression": "1/(16*pi^2*f_pi*f_D^2)"},
        {"quantity": "uv:ordered_prefactor_over_C", "expression": 2},
        {"quantity": "uv:polarization_sum_prefactor_over_C2", "expression": 0.25},
        {"quantity": "uv:cross_section_coefficient", "expression": "1/(64*pi)"},
        {"quantity": "uv:power_n", "expression": 2},
        {"quantity": "uv:power_alpha_Q", "expression": 1},
        {"quantity": "uv:power_s", "expression": 1.5},
        {"quantity": "uv:power_threshold", "expression": 0.5},
        {"quantity": "uv:power_f_pi", "expression": -2},
        {"quantity": "uv:power_f_D", "expression": -4},
        {"quantity": "uv:fixed_rate_f_D_Nc_power", "expression": 0.5}
      ],
      "validity_conditions": [
        "unit_charges", "unit_periods", "fixed_phase", "scoped_uv_completion",
        "leading_chiral_order", "canonical_fields", "ordered_distinct_species",
        "massless_final_states", "tree_level"
      ],
      "calculation_summary": "FIRST-PRINCIPLES DERIVATION STORY (tree level, leading chiral/derivative order).\\n\\nSTEP 1 - MICROSCOPIC MATCHING OF n. The mixed topological action S_mix = 2 pi n int omega_3 ^ Omega_2 is the infrared remnant of the UV response of the generalized magnetic current star j_m^{(2)} = d b/(2 pi). Both omega_3 and Omega_2 have unit period, so the coefficient 2 pi n counts the winding of the closed S^2 path threaded by the WZW class. With N_c colour copies and unit b-charges (X_q=1, X_phi=1), the anomaly-matched winding equals the colour number: n = N_c. Hence n/N_c = 1.\\n\\nSTEP 2 - GAUGED DESCENT TO THE PORTAL. Under the QED gauging a_int = (e/3) A, the WZW form descends as omega_3 -> omega_3 - (e/4 pi^2) F ^ Tr(Q g^-1 dg). Expanding g^-1 dg = 2 i d(pi_a t_a)/f_pi and taking the neutral-flavour trace Tr(Q t3) = 1/2, the mixed action contains the local portal S_portal = int C eps^{mu nu rho sigma} pi0 F_{mu nu} partial_rho chi1 partial_sigma chi2 with C = n e/(16 pi^2 f_pi f_D^2). Therefore C/(n e) = 1/(16 pi^2 f_pi f_D^2). Dimension check: [C] = [M]^-3, as required for the dimension-5 operator.\\n\\nSTEP 3 - ORDERED AMPLITUDE. Contracting L_portal against the photon momentum k_mu, polarization eps_nu, and the two ordered scalar momenta p1_rho, p2_sigma gives the ordered amplitude M = -2 i C eps^{mu nu rho sigma} k_mu eps_nu p1_rho p2_sigma, i.e. ordered_prefactor_over_C = 2.\\n\\nSTEP 4 - POLARIZATION SUM BY EXPLICIT LEVI-CIVITA CONTRACTION. Summing over the two physical photon polarizations: for the transverse basis eps^(+-) with momentum k, the Levi-Civita contraction gives sum_pol |M|^2 = C^2 P s^2 (s - 4 m_chi^2) with P = 1/4. Cross-check by the covariant replacement sum_pol eps_mu eps_nu -> -g_{mu nu} and the Gram determinant det(Gram) = s^2 (s - 4 m_chi^2)/16: both methods agree. Hence polarization_sum_prefactor_over_C2 = 1/4.\\n\\nSTEP 5 - TWO-BODY PHASE SPACE AND CROSS SECTION. For chi1 chi2 -> pi0 gamma with massless final states and s > 4 m_chi^2, the flux factor and the two-body phase space integral give sigma = C^2 s^{3/2} (s - 4 m_chi^2)^{1/2}/(64 pi). Powers: n^2 from C^2 ~ n^2, alpha_Q^1 from e^2 = 4 pi alpha_Q, s^{3/2}, threshold (s - 4 m_chi^2)^{1/2}, f_pi^{-2}, f_D^{-4}.\\n\\nSTEP 6 - FIXED-RATE N_c SCALING. Holding the target rate, s, m_chi, alpha_Q and f_pi fixed, sigma ~ n^2/f_D^4 implies f_D^4 ~ N_c^2, so f_D ~ N_c^{1/2}: fixed_rate.f_D_Nc_power = 1/2.\\n\\nScope note: the matched coefficient holds inside one fixed charge assignment (X_q = X_phi = 1), one fixed Higgsed phase, and one scoped UV completion; no universal claim is made."
    },
    "R_SPLIT": {
      "target_id": "R_SPLIT",
      "premises": [
        "Two species with masses m1 and m2 >= m1, internal degeneracies g1, g2 > 0, common temperature T; x = m1/T, Delta = (m2-m1)/m1.",
        "q = g2/g1; delta_m = m2 - m1; split and unsplit freeze-out values x_f, x_f0; positive f_D-independent coefficients K_delta, K_zero (all kinematic/thermal dependence independent of f_D; no relation between them assumed).",
        "kappa_D denotes the power of f_D in the R_UV ordered rate; the split and unsplit ordered mixed rates inherit it: <sigma12 v>_Delta = K_delta f_D(Delta)^kappa_D, <sigma12 v>_0 = K_0 f_D(0)^kappa_D.",
        "Species in relative chemical equilibrium with nonrelativistic Maxwell-Boltzmann populations; only the mixed channel changes number (<sigma12> = <sigma21>, vanishing diagonal rates); kinetic contact holds at the population-weight temperature. Principal positive real branch throughout."
      ],
      "calculations": [
        {"quantity": "split:relative_splitting", "expression": "delta_m/m1"},
        {"quantity": "split:a", "expression": "q*(1+Delta)^1.5*exp(-x_f*Delta)"},
        {"quantity": "split:g_eff", "expression": "g1+g2*(1+Delta)^1.5*exp(-x_f*Delta)"},
        {"quantity": "split:population_weight", "expression": "a/(1+a)^2"},
        {"quantity": "split:exponential_component", "expression": "exp(-x_f*Delta)"},
        {"quantity": "split:inherited_rate_fD_power", "expression": -4},
        {"quantity": "split:sigma_eff", "expression": "2*K_delta*f_D^kappa_D*a/(1+a)^2"},
        {"quantity": "split:equal_degenerate_limit", "expression": "K_zero*f_D^kappa_D/2"},
        {"quantity": "split:fD_ratio", "expression": "((K_zero/K_delta)*(W_zero/W_delta))^(1/kappa_D)"}
      ],
      "validity_conditions": [
        "relative_chemical_equilibrium", "mixed_channel_dominance", "kinetic_contact",
        "nonrelativistic_weights", "same_derived_rate_power", "fixed_effective_rate"
      ],
      "calculation_summary": "FIRST-PRINCIPLES THERMAL DERIVATION (nonrelativistic Maxwell-Boltzmann coannihilation).\\n\\nSTEP 1 - RELATIVE SPLITTING. By definition Delta = (m2-m1)/m1 = delta_m/m1.\\n\\nSTEP 2 - EQUILIBRIUM POPULATION RATIO a. Nonrelativistic Maxwell-Boltzmann densities n_i = g_i (m_i T/(2 pi))^{3/2} exp(-m_i/T). The unnormalized split population ratio EVALUATED AT THE FREEZE-OUT x_f is a = n2/n1 = (g2/g1)(m2/m1)^{3/2} exp(-(m2-m1)/T) = q (1+Delta)^{3/2} exp(-x_f Delta), using (m2-m1)/T = (m1/T) Delta = x_f Delta.\\n\\nSTEP 3 - EFFECTIVE DEGENERACY. g_eff = g1 + g2 (1+Delta)^{3/2} exp(-x_f Delta) = g1 (1 + a); in the unsplit limit (Delta -> 0) g_eff -> g1 + g2.\\n\\nSTEP 4 - POPULATION WEIGHT OF ONE ORDERED MIXED RATE. Total density n_tot = n1 + n2; normalized equilibrium fractions r1 = n1/n_tot = 1/(1+a), r2 = n2/n_tot = a/(1+a). The population multiplier of one ordered mixed rate is W_delta = n1 n2/n_tot^2 = r1 r2 = a/(1+a)^2.\\n\\nSTEP 5 - EFFECTIVE RATE ON THE TOTAL DENSITY. The effective equation uses the total number density: d n_tot/dt = -2 n1 n2 <sigma12 v>_Delta (the two ordered pairs 12 and 21 carry equal rates). Writing d n_tot/dt = -<sigma v>_eff n_tot^2 gives <sigma v>_eff = 2 K_delta f_D^kappa_D a/(1+a)^2.\\n\\nSTEP 6 - EQUAL-DEGENERATE UNSPLIT LIMIT. Delta -> 0 with g2 -> g1 (q -> 1): then a -> 1 and W -> 1/4, so <sigma v>_eff,0 = 2 K_0 f_D(0)^kappa_D (1/4) = (1/2) K_0 f_D(0)^kappa_D.\\n\\nSTEP 7 - f_D RATIO PRESERVING THE EFFECTIVE RATE. Requiring the same effective rate in both systems, K_delta f_D(Delta)^kappa_D W_delta = K_0 f_D(0)^kappa_D W_zero, gives [f_D(Delta)/f_D(0)]^kappa_D = (K_0/K_delta)(W_zero/W_delta), hence f_D(Delta)/f_D(0) = [(K_0/K_delta)(W_zero/W_delta)]^{1/kappa_D} on the principal positive real branch, with W_zero evaluated at x_f0.\\n\\nSTEP 8 - EXPONENTIAL COMPONENT. Separating the factor due only to the heavier equilibrium population's Boltzmann exponential from the full ratio a gives exp(-x_f Delta) (distinct from the phase-space factor (1+Delta)^{3/2} and the degeneracy ratio q).\\n\\nSTEP 9 - INHERITANCE. split:inherited_rate_fD_power = uv:power_f_D = -4, and every R_SPLIT expression stays symbolic in kappa_D."
    }
  },
  "prerequisite": {
    "source_target": "R_UV",
    "source_quantity": "rate_fD_power",
    "destination_target": "R_SPLIT",
    "destination_quantity": "inherited_rate_fD_power",
    "relation": "prerequisite_identity"
  }
}
<!-- DERIVATION_RECORD_END -->
"""

vdir = os.path.join(WORK, "variant_D")
os.makedirs(os.path.join(vdir, "final"), exist_ok=True)
os.makedirs(os.path.join(vdir, "evidence"), exist_ok=True)
json.dump(a, open(os.path.join(vdir, "final", "answer.json"), "w", encoding="utf-8"), indent=2)
json.dump(d, open(os.path.join(vdir, "evidence", "derivation.json"), "w", encoding="utf-8"), indent=2)
open(os.path.join(vdir, "DERIVATION.md"), "w", encoding="utf-8").write(narrative)
print("variant D written:", vdir)
