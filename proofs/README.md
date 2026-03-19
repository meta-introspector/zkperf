# Proof Artifacts

## Imported Proofs

### state-4-zkperf (consensus game state)
- `state-4-zkperf.json` — Game state with consensus, ships, zkperf commitments
- `state-4-zkperf.perf.txt` — Actual perf stat output (1.9M cycles, 2.7M instructions)
- `state-4-zkperf.commitment` — `fab3ece438dd78f2cdcd645b984885ebd24ed4c3f8c4f2bfca87f56fd63a59d7`

### harbot-proof-system
- `conformal_witness.json` — Python ≅ Rust via Monster group conformal mapping (shard 41/prime 181, shard 30/prime 127)
- `NO_OLD_CODE_MANIFEST.json` — Iteration 4 proof: Lean4 ≡ Coq ≡ Prolog (UniMath-style), no old code referenced

### onlyskills_integration (pipelight-dev)
10 component proofs with commitments and witness verification:

| Component | File |
|---|---|
| parser | `pipelight-dev_parser_0.json` |
| compiler | `pipelight-dev_compiler_1.json` |
| optimizer | `pipelight-dev_optimizer_2.json` |
| analyzer | `pipelight-dev_analyzer_3.json` |
| transformer | `pipelight-dev_transformer_4.json` |
| validator | `pipelight-dev_validator_5.json` |
| generator | `pipelight-dev_generator_6.json` |
| executor | `pipelight-dev_executor_7.json` |
| tracer | `pipelight-dev_tracer_8.json` |
| profiler | `pipelight-dev_profiler_9.json` |

Each contains: `{statement, commitment, witness: "hidden", verified: true, timestamp}`
