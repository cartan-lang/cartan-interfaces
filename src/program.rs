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

/// One thing a supplier can be asked to compute.
///
/// A State's name means the value it has now. Its initial value, and the
/// value an update gives it, are different computations, so each has its own
/// way to be selected. What is selected says nothing about what is done with
/// the result.
///
/// Gap: a value that the source does not name, such as an intermediate one
/// unit computes for another to take as an input, cannot be selected yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Root {
    /// The value a declaration has: a binding's value, a Param, or a State as
    /// it stands.
    Value(Name),
    /// The value a State starts with. Initial values are closed: they read no
    /// State and no Param. A State that starts absent and is later given a
    /// field computed from Params gets that field from an ordinary binding,
    /// not from its initial value.
    Initial(StateId),
    /// The value one update block gives one of the States it writes, computed
    /// from the State as it stands.
    Next { update: UpdateId, state: StateId },
}

/// One update block and the States it writes.
#[derive(Clone, Debug)]
pub struct Update {
    pub id: UpdateId,
    pub writes: Vec<StateId>,
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
    /// Update blocks, in the order the source writes them. That is not the
    /// order in which they run. Everything one block writes on one execution
    /// takes effect together.
    pub updates: Vec<Update>,
}
