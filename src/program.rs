//! The nodes of a program's dependency graph, and what a source calls them.

use crate::kind::{KindRef, Kinds};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);

/// The identity of one derived value. Identical computations have the same
/// identity, whatever the source calls them and whether or not it names them
/// at all.
///
/// Gap: what makes two computations identical is decided where the source is
/// analyzed and is not written here. An identity holds within one analyzed
/// program; whether it survives a change to the source is not settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UpdateId(pub u32);

/// One node of a program's dependency graph: something that has a value at
/// each version.
///
/// A State and a Param are identified by name. Two with the same
/// declaration are still different, because each has its own history. A
/// derived value is identified by what it computes. Two bindings that
/// compute the same thing are one node with one value, and a value the
/// source never names is a node like any other.
///
/// A node is the same everywhere it appears: in a unit's ports, in what the
/// runtime holds and in what is selected. Port order and type numbering are
/// never identity. A node does not identify one value: it has a value at
/// each version and in each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    /// Written by update blocks; belongs to the run.
    State(StateId),
    /// Set from outside the run; never written by an update.
    Param(ParamId),
    /// A pure function of States, Params and other derived values. Never
    /// written. A unit may take one as an input once its value has been
    /// computed.
    Derived(DerivedId),
}

/// One name the source gives a node: the node, its spelling and its kind.
/// Several names may be given to one derived value.
#[derive(Clone, Debug)]
pub struct Declared {
    pub node: Node,
    pub label: String,
    /// A reference into the program's table of kinds.
    pub kind: KindRef,
}

/// One thing a supplier can be asked to compute.
///
/// A State stands for the value it has now. Its initial value, and the value
/// an update gives it, are different computations, so each has its own way
/// to be selected. What is selected says nothing about what is done with the
/// result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Root {
    /// The value a node has: a derived value, a Param, or a State as it
    /// stands.
    Value(Node),
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
    /// Every kind the declared nodes use, each stated once.
    pub kinds: Kinds,
    pub declared: Vec<Declared>,
    /// Update blocks, in the order the source writes them. That is not the
    /// order in which they run. Everything one block writes on one execution
    /// takes effect together.
    pub updates: Vec<Update>,
}
