//! The draft contracts between Cartan components.
//!
//! Each module is one component's side: the types it owns and what it offers
//! the others. Everything is plain description with no dependencies.
//!
//! The crate has two layers. The runtime-facing modules, `program`, `unit`,
//! `supply` and `session`, are everything the runtime needs from a compiler
//! and everything an application needs from the runtime. They import nothing
//! from the other modules, so a compiler built from other components can
//! implement [`supply::Supplier`] and be run by the same runtime. Building
//! with `--no-default-features` compiles this layer alone.
//!
//! The `pipeline` modules, `syntax`, `graph`, `codegen` and `compile`, are the
//! contracts between the components of the official compiler.
//!
//! A code generator describes each piece of compiled code as a [`unit::Unit`]:
//! the values it reads, the values it produces, the storage it needs and the
//! stages it runs. The runtime owns everything live. It binds inputs, provides
//! storage, runs the stages and publishes results. Compiled code never
//! allocates memory and never publishes.
//!
//! Everything here is a draft. Doc paragraphs starting `Gap:` name parts of a
//! contract that are not written yet; paragraphs starting `Decide:` name open
//! choices.

// The runtime-facing layer.
pub mod program;

pub mod unit;

pub mod supply;

pub mod session;

// The official compiler's pipeline, in order.
#[cfg(feature = "pipeline")]
pub mod syntax;

#[cfg(feature = "pipeline")]
pub mod graph;

#[cfg(feature = "pipeline")]
pub mod codegen;

#[cfg(feature = "pipeline")]
pub mod compile;
