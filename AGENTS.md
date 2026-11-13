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

- Own the contracts between Cartan components: plain types and trait
  signatures, one module per component's side. The only logic is checking that
  a description is well formed. The crate has no dependencies.
- Keep the two layers. The runtime-facing modules (`program`, `unit`, `supply`,
  `session`) import nothing from the pipeline modules (`syntax`, `graph`,
  `codegen`, `compile`), which sit behind the default `pipeline` feature.
- A change here is an agreement between the components on both sides of a
  boundary. Agree it with their owners before committing, and with the
  workspace owner for the runtime-facing layer.
- Keep the crate publishable on its own. State rationale directly. Do not refer
  to mechanisms inside other components, to workspace records or to people.
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
cargo check --no-default-features
python3 tools/docs.py check --strict
```
