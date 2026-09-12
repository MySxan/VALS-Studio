# VALS Studio implementation rules

- `docs/architecture/Vocal_Analysis_Software_Design.md` is an architecture constraint. Do not change core boundaries without an explicitly approved ADR.
- Before implementing a stage, briefly state its crates/modules, public interfaces, invariants, cache/analysis dependencies and tests; then implement a runnable, reviewable vertical slice.
- `VocalProject` is the sole semantic project truth. Application/IPC/UI projections and derived analysis artifacts must remain separate.
- Analysis must be provider-independent, cancellable, cacheable, replaceable, and retain confidence and provenance. Never overwrite user overrides. Keep Raw Observation, Resolved Value and User Override separate.
- Concrete models and third-party libraries belong in adapters/providers, not the Canonical Domain Model.
- Prefer minimal repairs. Do not change persistent schema or domain semantics incidentally to UI work. Preserve migration and atomic-save contracts.
- Run relevant checks and fix regressions introduced by the stage. Report completed work and remaining limitations briefly.
- Update `docs/CURRENT.md` with actual capabilities, stable interfaces/decisions and known debt. Update `docs/NEXT.md` with the next goal, missing prerequisites and recommended vertical slice. Keep both short; do not use them as development logs.
