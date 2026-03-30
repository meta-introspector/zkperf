# MDL Evidence v1

`mdl-evidence-v1` is a bounded JSON contract for attaching machine-readable
MDL witnesses to a run without requiring every downstream consumer to know a
repo-specific internal report shape.

It is intentionally small:

- one program / run / input identity surface
- one declared family / model class / coding scheme
- one concrete MDL witness block
- optional trajectory and witness provenance

This is useful for:

- attaching MDL evidence to a perf or witness run
- comparing repeated runs of the same program family
- feeding external observability layers without overclaiming universal MDL

This is not a claim that MDL is an intrinsic free-floating property of an
arbitrary program. It is always relative to a declared coding regime.

## Minimal JSON shape

```json
{
  "schema_version": "mdl-evidence-v1",
  "program_id": "my-program",
  "run_id": "run-2026-03-30T12:34:56Z",
  "input_id": "input-abc123",
  "family": "z_pt_7tev_atlas",
  "model_class": "my_model_family_v1",
  "coding_scheme": "exact_code_length_v1",
  "measured_at": "2026-03-30T12:34:56Z",
  "mdl": {
    "total_length": 1234.5,
    "descent_monotone": false,
    "violation_count": 3,
    "worst_increase": 12.25,
    "worst_step": 17
  },
  "trajectory": [
    { "step": 0, "length": 1300.0 },
    { "step": 1, "length": 1288.5 },
    { "step": 2, "length": 1290.0 }
  ],
  "witness": {
    "artifact_path": "artifacts/mdl/run-123.json",
    "commit": "abcdef123456",
    "notes": "optional free text"
  }
}
```

## Required fields

- `schema_version`
- `program_id`
- `run_id`
- `family`
- `coding_scheme`
- `mdl.total_length`
- `mdl.descent_monotone`
- `mdl.violation_count`
- `mdl.worst_increase`

## Recommended fields

- `input_id`
- `model_class`
- `measured_at`
- `mdl.worst_step`
- `trajectory`
- `witness`

## Semantics

- `mdl.total_length`
  - actual description length under the declared coding scheme
- `mdl.descent_monotone`
  - whether the trajectory is non-increasing
- `mdl.violation_count`
  - number of upward descent violations
- `mdl.worst_increase`
  - largest upward step in the observed descent trace
- `mdl.worst_step`
  - optional index of the worst increase

## Governance notes

- comparisons are only honest when the coding scheme and family are declared
- this shape is meant for bounded interoperability, not universal MDL claims
- a future function-relative empirical surface can coexist with this, but it
  should be named differently and must not be conflated with formal MDL
