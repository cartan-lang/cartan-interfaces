# Working in cartan-interfaces

Read README and docs/component.md first. Shared rules, workflow commands,
ownership and the HANDOFF shape are in the workspace
[AGENTS.md](../AGENTS.md); joint Codex/Claude operation, ownership handback and
mailboxes are in [joint operation](../docs/records/joint-operation.md). In a
standalone clone, the essentials are:

- One agreed writer per checkout, recorded in the ignored
  `.agent-mail/ownership.json`; others review read-only until it is handed back.
- Commits: sentence-case imperative subjects of about 60 characters, no trailing
  period or prefix, the configured Git identity, no agent attribution or
  co-author trailers; stage explicit paths; no push or publication without owner
  authorization.
- Documentation follows [docs/documentation.md](docs/documentation.md): current
  state only, one home per fact, done work items deleted after consolidation.
- HANDOFF.md is ignored session state, never committed.
- Design from requirements before accommodating what exists, and escalate when
  an architectural change makes an obviously simpler contract possible.

## Scope

- The contracts are aspirational and deliberately idealistic. They state what
  should cross each boundary, designed from requirements. Component source is
  brought into compliance with them; they are never bent to fit existing code.
- Build them out gradually, one owner ruling at a time. An under-specified
  contract is acceptable. A `Gap:` is intentional, not a task. Existing code
  and older ABI or implementation records are not authority for a contract.
- A request to revise a contract arrives as an escalation from a builder who
  cannot meet a requirement within it. Take it to the workspace owner with the
  requirement, what the contract cannot express and the smallest change.
- Own the contracts between Cartan components: plain types and trait
  signatures. The only logic is checking that a description is well formed.
  The crate has no dependencies.
- Keep the declarations in one file. `src/lib.rs` holds every type and trait
  with its documentation, in reading order, and no `impl`. Every `impl`,
  including the checks, goes in `src/checks.rs`, and tests in `src/tests.rs`.
- State only what crosses a boundary: between a compiler and a runtime, and
  between an application and a runtime. Nothing inside a compiler or inside a
  runtime belongs here.
- Keep one vocabulary. Make a term a type wherever it can be one, and define
  it once, in that type's documentation. Define a word that cannot be a Rust
  item once, in the documentation of the type it bears on.
- A change here is an agreement between the components on both sides of a
  boundary. Agree it with their owners before committing, and with the
  workspace owner for the runtime-facing layer.
- Keep the crate publishable on its own. State rationale directly. Do not refer
  to mechanisms inside other components, to workspace records or to people.
- A contract must let shared structure be stated once and referred to. Never
  add a form that has to write a shared thing out at each use, or that rules
  out hash-consing or hoisting on either side. Check each addition against
  nested Structs with Array members and against an expression used in both
  branches of a conditional.
- Mark parts not yet written with a doc paragraph starting `Gap:` and open
  choices with `Decide:`, so `grep` lists every open point.
- Implementations of a component's insides do not belong here. Tests here
  cover only the well-formedness checks.
- This component has no `cli` package. The owner exempted it on 2026-11-13
  because it exposes no operations.

## Checks

Run before every commit:

```sh
cargo fmt --check
cargo test --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
# From the workspace root:
uv run --no-sync cartdev docs check --root cartan-interfaces --strict
```
