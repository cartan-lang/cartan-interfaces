---
id: component
title: Interfaces component
kind: reference
status: accepted
implementation: partial
topics: [interface, architecture]
---

# Interfaces component

This component holds the contracts between Cartan components as Rust types and
trait signatures. It contains descriptions only: no evaluator, compiler, runtime
or allocator. It has no dependencies and no command-line program.

Every contract is a draft. No component depends on this crate yet. A boundary is
adopted when the components on both sides import its types and delete their own
equivalents.

## Layers

The runtime-facing modules are everything a runtime needs from a compiler and
everything an application needs from a runtime:

- `kind` says what a value is: fibers, Arrays with the form of each axis, and
  Optional values. It holds the language's kinds, not the rules that assign them.
- `program` names what a source declares: States, Params, bindings and update
  blocks.
- `unit` describes one piece of compiled code: its inputs and results, their
  kinds, sizes and physical layout, the storage it needs and in which memory,
  and the stages it runs.
- `supply` is what a compiler offers a runtime: units on request.
- `session` is what an application asks of a runtime and what a runtime reports.

These modules import nothing from the others. A compiler built from other
components implements `Supplier` and is run by the same runtime.
`cargo check --no-default-features` builds this layer alone.

The pipeline modules are the contracts between the components of the official
compiler, behind the default `pipeline` feature: `syntax`, `graph`, `codegen`
and `compile`. Most are placeholders.

## Open points

Open points live in the source. A doc paragraph starting `Gap:` names a part of
a contract that is not written yet. One starting `Decide:` names an open choice.
`grep -rn "Gap:\|Decide:" src` lists them all.

## Checks

`cargo fmt --check`, `cargo test --locked`, `cargo check --no-default-features`
and `python3 tools/docs.py check --strict`.
