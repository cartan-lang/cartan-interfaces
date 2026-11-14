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

## Layout

`src/lib.rs` holds every declaration: each type and trait with its
documentation, in reading order, and no `impl`. Each term the contracts use is a
type wherever it can be one, and is defined once, in that type's documentation.
Every `impl`, including the checks that a description is well formed, is in
`src/checks.rs`. Tests are in `src/tests.rs`.

## Layers

The runtime-facing sections come first. They are everything a runtime needs from
a compiler and everything an application needs from a runtime:

- Kinds say what a value is: fibers, Arrays with the form of each axis,
  Optional values, Structs and Tuples. They are the language's kinds, not the
  rules that assign them. Kinds live in a table and are referred to by their
  place in it, so a kind used in many places is stated once. Naming a fiber
  obliges no component to support it; where the storage of a fiber is not
  described, a unit carrying it is refused as unsupported.
- Layouts say how a value is kept in storage: which planes it uses and where
  each part sits in an element. A layout follows the kind's structure and is
  stated once in the same way, so a layout that repeats is written once.
- Programs name what a source declares: States, Params, bindings and update
  blocks.
- Units describe one piece of compiled code: its input and output ports, their
  kinds, shape and physical layout, the storage it needs and in which memory,
  and its stages. A stage is one launch of compiled code and the only
  execution boundary. A stage consumes only what was produced before it,
  shape values included, and each shape value has one producer. A shape value
  is a List length, an Array domain or an Optional's presence.
- Supply is what a compiler offers a runtime: a unit that meets the demands
  made of it.
- Sessions are what an application asks of a runtime and what a runtime
  reports.

These sections use nothing from the pipeline sections. A compiler built from
other components implements `Supplier` and is run by the same runtime.
`cargo check --no-default-features` builds this layer alone.

The pipeline sections come last and are the contracts between the components of
the official compiler, behind the default `pipeline` feature: syntax, graph,
code generation and preparation. Most are placeholders.

## Open points

Open points live in the source. A doc paragraph starting `Gap:` names a part of
a contract that is not written yet. One starting `Decide:` names an open choice.
`grep -rn "Gap:\|Decide:" src` lists them all.

## Checks

`cargo fmt --check`, `cargo test --locked`, `cargo check --no-default-features`,
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` with and without
`--no-default-features`, and `python3 tools/docs.py check --strict`.
