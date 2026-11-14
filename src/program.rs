//! What a program lets you name.

use crate::kind::{KindRef, Kinds};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BindingId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UpdateId(pub u32);

/// What the source calls a declaration.
///
/// A name identifies a declaration everywhere: in a unit's ports, in the
/// runtime's committed State and in a request. Port order and type numbering
/// are never identity. A name does not identify one value: the same State has
/// a different value at each version and in each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Name {
    /// Written by update blocks; belongs to the run.
    State(StateId),
    /// Set from outside the run; never written by an update.
    Param(ParamId),
    /// Derived from States, Params and other bindings. Read-only. A unit may
    /// take one as an input once its value has been computed.
    Binding(BindingId),
}

/// One nameable value: its identity, its spelling in the source and its type.
#[derive(Clone, Debug)]
pub struct Declared {
    pub name: Name,
    pub label: String,
    /// A reference into the program's table of kinds.
    pub kind: KindRef,
}

/// Everything one analyzed source declares.
///
/// The supplier assigns the ids. The runtime treats them as opaque and uses
/// the labels only to resolve what a user asks for.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Every kind the declarations use, each stated once.
    pub kinds: Kinds,
    pub declared: Vec<Declared>,
    /// Update blocks, in source order. Each execution of one is one atomic batch.
    pub updates: Vec<UpdateId>,
}
