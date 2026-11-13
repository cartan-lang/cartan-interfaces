//! What a compiler front half supplies to the runtime.

use crate::program::{Name, Program, UpdateId};
use crate::unit::{Checked, Code, Target};

/// What the runtime asks a supplier to compile.
///
/// All three are the same kind of computation: read named values, produce
/// named values. They differ only in which values are produced.
#[derive(Clone, Copy, Debug)]
pub enum Request<'a> {
    /// The initial value of every State. Initial values are closed: they read
    /// no State and no Param. A State that starts absent and is later given a
    /// field computed from Params gets that field from an ordinary binding,
    /// not from its initial value.
    Launch,
    /// One execution of one update block. Reads the committed State, Params
    /// and upstream bindings, and produces the next value of each State the
    /// block writes.
    Update(UpdateId),
    /// The values of the named bindings for one committed State. Reads the
    /// same things an update may. Produces no State. The names need not have
    /// been selected at launch.
    Observe(&'a [Name]),
}

/// A checked unit description and the target code it describes.
pub struct Emitted {
    pub unit: Checked,
    pub code: Code,
}

/// The front half of a compiler, as the runtime sees it.
///
/// The runtime needs nothing else from a compiler: no syntax tree, no semantic
/// graph and no knowledge of how the code was generated. Two suppliers that
/// honor this contract are interchangeable to the runtime.
///
/// An input port may name a binding. The supplier chooses, binding by
/// binding, between computing it inside the unit and having it computed once
/// and passed in. This is the same kind of choice as fusing or separating
/// the steps within a unit, made at the unit's edge. When a unit asks for a binding as an input, the runtime
/// must hold that binding's value first, requesting it with `Observe` if it
/// does not.
///
/// Decide: how the runtime knows a held binding is still good for the State it
/// is about to read. The simplest rule is that a binding's value stays valid
/// while every value it was computed from is unchanged.
///
/// Decide: whether a supplier may be asked for a unit while earlier units from
/// it are running. The runtime needs this for a request made after launch.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    fn unit(&self, request: Request<'_>, target: Target) -> Result<Emitted, Self::Diagnostic>;
}
