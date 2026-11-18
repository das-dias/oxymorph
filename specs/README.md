# Specs

What the code **already does**, in the present tense. A spec never describes
something that is not built yet. Rationale lives in
[`../decisions/`](../decisions/); plans and sequencing live in
[`../ROADMAP.md`](../ROADMAP.md).

Ledgers (e.g. a known-issue table) are allowed here: they describe the current
state, not a plan.

## Relationship to decisions

- `decisions/` records **why** we chose something, once, and is frozen.
- `specs/` records **what** is true now, and evolves.
- Every spec section that encodes a non-obvious choice should cite the ADR that
  justifies it (e.g. `see decisions/0001`).

## Conventions

- One spec per bounded area, named `<area>.md` (e.g. `ir.md`, `parsing.md`).
- Each spec states: scope, requirements (numbered, testable), invariants, and
  open questions.
- Requirements are numbered (`R1`, `R2`, …) so ADRs, tests, and issues can
  reference them.
- When a requirement is implemented, link the test(s) that verify it.
- Update the spec in the same commit as the code it describes.

## Index

| Spec | Covers |
|------|--------|
| [parity.md](parity.md) | The equivalence contract, corpus, verification mechanisms, and the known-upstream-issues ledger |
