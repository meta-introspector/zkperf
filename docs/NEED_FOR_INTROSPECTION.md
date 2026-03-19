# The Need for Introspection

> CID: `bafkd86ff39b6db55eeca7ee83de2f8807f2`
> Witness: `d86ff39b6db55eeca7ee83de2f8807f22eb8f09af665b5fc3c7104391415a734`
> IPFS: `QmebWvG7MamYznHzFKAb3Y9vyhtyS13R3tMkbkvcbQqn6a`
> DASL: `0xda5130334089b6db`
> Sheaf: `11,29,24 H/raw p=1 T3 Earth B3 T_1`

## The Problem

A perf witness to be reproducible has higher standards. We need a way to share and prove we have the full chain.

## The Five Layers

A reproducible perf witness must bundle:

1. **The nix package of binaries** — the exact store paths, closure, and all transitive dependencies
2. **Source and debug symbols** — so traces can be mapped back to code lines, functions, and call graphs
3. **Traces produced with the binaries** — perf.data, strace logs, stat counters — the raw observations
4. **Models created from the traces** — derived metrics, comparisons, complexity claims, conformal mappings
5. **Events leading up to binary creation** — git history, CI logs, build environment, the provenance chain

Without ALL FIVE, a witness is incomplete. Anyone can claim "1.9M cycles" but can they prove:
- Which exact binary produced those cycles?
- What source code compiled to that binary?
- That the trace wasn't tampered with?
- That the model faithfully represents the trace?
- That the build environment was clean?

## Implementation

### Bash: Full chain recording
```bash
./examples/zkperf-full-chain.sh /nix/store/slnid5pk8zci6xvszn4y306wpzhbvpyy-state-4-zkperf .
```

### Python: Chain assembly + commitment
```bash
python3 examples/zkperf-chain.py /nix/store/slnid5pk8zci6xvszn4y306wpzhbvpyy-state-4-zkperf src/witness.rs recordings/rust_actual.perf.data
```

### JavaScript: Chain verification
```bash
node examples/zkperf-verify.js proofs/witness-chain.json
```

## Chain Structure

```
Layer 1: Binaries ──────────┐
Layer 2: Source + Debug ─────┤
Layer 3: Traces ─────────────┼──→ Chain Hash ──→ Commitment
Layer 4: Model ──────────────┤
Layer 5: Events ─────────────┘
```

Each layer is independently hashable. The commitment is the hash of all layers combined. Verification checks that every layer is present and the commitment matches.

## eRDFa Sheaf Encoding

The witness chain maps to the eRDFa sheaf structure:

```html
<div typeof="erdfa:SheafSection dasl:Type3" about="#bafkda5130334089b6db">
  <meta property="erdfa:shard" content="11,29,24" />
  <meta property="erdfa:encoding" content="raw" />
  <meta property="erdfa:prime" content="1" />
  <meta property="dasl:addr" content="0xda5130184089b6db" />
  <meta property="dasl:type" content="3" />
  <meta property="dasl:eigenspace" content="Earth" />
  <meta property="dasl:bott" content="3 (H⊕H)" />
  <meta property="dasl:hecke" content="T_1" />
  <meta property="sheaf:orbifold" content="(11 mod 71, 29 mod 59, 24 mod 47)" />
  <link property="sheaf:subgroupIndex" href="erdfa:H/raw" />
</div>
```

The orbifold coordinates `(11 mod 71, 29 mod 59, 24 mod 47)` map to the same sector space as the consensus game state — sector 71 (alpha), sector 59 (beta), sector 47 (near gamma at 2).
