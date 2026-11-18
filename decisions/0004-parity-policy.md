# 0004. Parity policy and upstream divergence log

- Status: implemented
- Date: 2026-02-10
- Supersedes: -
- Superseded by: -
- Related decisions: `0001-rust-port-of-morphnet.md`
- Related specs: `specs/parity.md`

## Context

Decision 0001 sets an initial goal of **functional equivalence** with the
vendored Python morphnet, to be verified by a golden corpus plus a
non-blocking differential harness. During a port, the two implementations will
sometimes disagree because *upstream is wrong* (a bug), not because the port is.

Without an explicit policy, such a discovery has no defined resolution: the port
either silently diverges (making the parity harness permanently red with
unexplained deltas) or silently reproduces the bug with no record of it.

## Decision

**Bug-for-bug parity.** While the port is tracking upstream, the Rust
implementation reproduces upstream's observable behavior **exactly, including
bugs**. A disagreement with upstream is treated as a port defect unless it is
recorded as a known upstream issue.

- Discovered upstream bugs are **logged, not fixed**.
- Fixing one is a **deliberate divergence**: it requires a new ADR that changes
  this policy for that issue (or generally), after which the entry's status moves
  from `open` to `diverged` and it starts being excluded from parity.
- Fixes are expected to be cheap to land upstream, so batching/deferring them is
  acceptable; upstream fixes move the entry to `fixed-upstream` and the port
  then tracks the fixed behavior.

The log is a **living artifact** and therefore lives in `specs/parity.md` under
"Known upstream issues" — `specs/` evolves, `decisions/` is frozen.

## Options considered

- **(1) Bug-for-bug parity (chosen)** — parity stays a single hard invariant;
  divergences are explicit and authorized.
- **(2) Fix-forward** — log and fix in Rust immediately; simpler for obviously
  wrong behavior, but makes "exact match" unverifiable and turns the harness red
  by design.
- **(3) Fix-forward for crashes/loss only** — pragmatic middle ground; rejected
  because it splits behavior into two undocumented tiers, which is harder to
  reason about than a single policy plus recorded exceptions.

## Consequences

Easier: the parity contract is a clean invariant; every delta is either a defect
or a recorded, authorized divergence.

Harder / owed:

- Some Rust code will intentionally encode behavior we know is wrong; those sites
  must reference their log id (e.g. `// parity: UPSTREAM-0007`) so they are
  findable when the divergence ADR lands.
- The differential harness must support an **allow-list** of known upstream
  issues (xfail-style), or it will report recorded bugs as failures.
- Log entries are on the critical path of the port; they must be created at the
  moment of discovery, not retroactively.

## Verification

- Every intentional deviation in the Rust code carries a `// parity: <id>`
  comment resolving to a `specs/parity.md` entry.
- The differential harness has an explicit known-issue allow-list; removing an
  entry requires the corresponding divergence ADR.
- A log entry referencing a divergence without an ADR is a review blocker.
