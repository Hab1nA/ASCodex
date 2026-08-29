---
name: sequence-align
description: "Perform multiple sequence alignment and phylogenetic tree construction for biological sequences: validate input FASTA files, run BLAST homology searches, align with MUSCLE/MAFFT/Clustal Omega, assess alignment quality, trim gaps, build ML or Bayesian trees, and produce reproducibility logs. Trigger on: 'align these sequences', 'run MSA', 'multiple sequence alignment', 'build a phylogenetic tree', 'BLAST this sequence', 'find homologs', 'sequence identity matrix', 'run MUSCLE', 'MAFFT alignment', 'compare protein sequences', 'align DNA sequences', 'phylogeny analysis'. Also activates when a user uploads FASTA files for comparison, wants to reproduce a published phylogenetic tree, or needs to assess conservation across a protein family."
---

# /sequence-align — Biological Sequence Alignment & Phylogenetic Analysis

Multiple sequence alignment (MSA) and phylogenetic tree construction for biological sequences. Wraps standard tools (MUSCLE, MAFFT, Clustal Omega, BLAST) with validation, visualization, and reproducibility logging. Supports protein, DNA, and RNA sequences.

## Trigger

User mentions: "sequence alignment", "MSA", "multiple alignment", "phylogenetic tree", "BLAST", "MUSCLE", "MAFFT", "Clustal", "sequence comparison", "homology search", "align sequences", "sequence identity".

## Workflow

### Step 1 — Input Validation

Before alignment, verify the input sequences:

1. **Format check**: accept FASTA, GenBank, EMBL, or plain sequences. Convert to FASTA if needed.
2. **Sequence type**: detect DNA, RNA, or protein automatically. Flag mixed-type inputs.
3. **Quality checks**:

| Check | Action |
|-------|--------|
| Ambiguous bases > 5% | Warn: poor sequence quality |
| Sequence length variance > 10× | Warn: may need trimming |
| Duplicate sequences | Flag: identical entries |
| Non-standard characters | Error: clean before alignment |
| Fewer than 3 sequences | Warn: statistical power limited |

4. **Metadata**: record accession numbers, organism, gene name, sequence length for each input.

### Step 2 — Database Search (if needed)

If the user has a query sequence and needs homologs:

1. **BLAST search** against appropriate database:
   - Protein: UniProt/Swiss-Prot (curated) or nr (comprehensive)
   - Nucleotide: nt, RefSeq, or organism-specific databases
   - Structure: PDB for structural homologs

2. **Filter hits**:
   - E-value threshold (default: 1e-5, adjust per use case)
   - Sequence identity range (e.g., 30-90% for diverse MSA)
   - Coverage threshold (e.g., > 80% query coverage)
   - Taxonomic filtering if needed

3. **Document search parameters**:
```yaml
blast_search:
  program: blastp
  database: UniProt/Swiss-Prot (2024-03)
  query: P12345
  evalue: 1e-5
  identity_range: [30%, 90%]
  coverage_min: 80%
  hits_retained: 45 / 1203
```

### Step 3 — Multiple Sequence Alignment

Choose alignment tool based on dataset characteristics:

| Scenario | Recommended Tool | Rationale |
|----------|-----------------|-----------|
| < 500 sequences, any length | MUSCLE v5 | Best accuracy for small-medium sets |
| 500-10,000 sequences | MAFFT (L-INS-i) | Scales well, good accuracy |
| > 10,000 sequences | MAFFT (FFT-NS-2) | Fast, reasonable accuracy |
| Structural alignment needed | MAFFT (--add existing) + structures | Uses 3D information |
| Transmembrane proteins | PRALINE + TM topology | Handles TM regions |

Run alignment with:
1. Default parameters first
2. Inspect alignment quality (Step 4)
3. Re-run with adjusted parameters if needed

### Step 4 — Alignment Quality Assessment

Evaluate the MSA before using it:

1. **Column-level metrics**:
   - Conservation score per position (Shannon entropy or sum-of-pairs)
   - Gap fraction per column (flag columns with > 50% gaps)
   - Mark conserved blocks vs. variable regions

2. **Sequence-level metrics**:
   - Pairwise identity matrix
   - Sequences that align poorly (< 20% identity to consensus) — possible misalignment or distant homolog

3. **Visual inspection**: render alignment with colored residues

```
Position:  1234567890
Seq1/Hu:   MVLSPADKT-
Seq2/Mo:   MVLSGEDKS-
Seq3/Ra:   MVLSGDKKT-
Seq4/Ch:   MVL-AAWGKV
Consensus: MVL.....K.
Conserv:   ***. ...*
```

4. **Trim if needed**: remove columns with > 70% gaps using trimAl or Gblocks. Document what was trimmed.

### Step 5 — Phylogenetic Analysis (if requested)

1. **Model selection**: use ModelTest-NG or IQ-TREE's model finder to select the substitution model
   - Protein: WAG, LG, JTT (test with BIC/AIC)
   - DNA: GTR+Γ, HKY, TN93

2. **Tree construction**:
   - Quick: Neighbor-Joining (MEGA, quicktree)
   - Standard: Maximum Likelihood (IQ-TREE, RAxML)
   - Bayesian: MrBayes or BEAST (if divergence times needed)

3. **Branch support**: 1000 ultrafast bootstraps (IQ-TREE) or 100+ standard bootstraps (RAxML)

4. **Tree visualization**: Newick format + rendered image with:
   - Bootstrap values on nodes (show if ≥ 70%)
   - Branch lengths proportional to substitutions
   - Colored by clade / taxonomy / phenotype

### Step 6 — Report

```markdown
## Alignment Summary

**Sequences**: N sequences, L columns after trimming
**Tool**: MAFFT v7.520 (L-INS-i)
**Trimming**: trimAl (gap threshold 0.7)

### Quality Metrics
- Mean pairwise identity: X%
- Conserved columns (> 90% identity): Y / L
- Gap-rich columns removed: Z

### Phylogenetic Tree (if computed)
- Method: IQ-TREE 2 (ML), model LG+G4 (BIC selected)
- Bootstrap: 1000 ultrafast replicates
- Key clades: [describe major groupings]

### Files
- `alignment.fasta` — trimmed MSA
- `alignment_full.fasta` — untrimmed MSA
- `tree.nwk` — Newick tree
- `tree.png` — rendered tree figure

### Reproducibility
- All commands logged in `trace.md`
- Random seed: [value]
- Software versions: [list]
```

## Common Pitfalls

- **Aligning divergent sequences without structural information.** Below ~25% identity, sequence-only alignment is unreliable. Use structure-guided alignment.
- **Using wrong substitution model.** Run model selection; don't assume JTT for proteins or JC69 for DNA.
- **Ignoring long-branch attraction.** Highly divergent sequences can cluster artifactually. Use midpoint rooting cautiously.
- **Over-trimming.** Aggressive trimming removes real signal. Compare trees from trimmed vs. untrimmed alignments.
- **Treating bootstrap values as probabilities.** Bootstrap > 70% indicates support, not certainty. Report values honestly.
- **Mixing partial and full-length sequences.** Partial sequences cause excessive gaps. Trim to shared region or handle separately.
- **Forgetting to root the tree.** Unrooted trees cannot infer evolutionary direction. Use an outgroup or midpoint rooting.

## Supported Tools

- **Alignment**: MUSCLE v5, MAFFT v7, Clustal Omega, T-Coffee
- **Trimming**: trimAl, Gblocks
- **Search**: BLAST+, DIAMOND, MMseqs2
- **Phylogenetics**: IQ-TREE 2, RAxML-NG, MrBayes, BEAST2
- **Visualization**: iTOL, FigTree, ggtree (R), ETE Toolkit (Python)
