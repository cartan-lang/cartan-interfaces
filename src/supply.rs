//! What a compiler front half supplies to the runtime.

use crate::program::{Program, Root};
use crate::unit::{Checked, Code, Target};

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
/// The runtime selects what is to be computed and the supplier compiles it.
/// Every selection is the same kind of computation: read values, produce
/// values. What the runtime then does with the results is its own business
/// and no part of the selection. It makes the first State from every State's
/// initial value. It commits an update by taking the new values of all the
/// States the block writes together. It reports an observation from the
/// values of the declarations it was asked for.
///
/// An input port may be a binding. The supplier chooses, binding by binding,
/// between computing it inside the unit and having it computed once and
/// passed in. This is the same kind of choice as fusing or separating the
/// steps within a unit, made at the unit's edge. When a unit lists a binding
/// as an input, the runtime must hold that binding's value first, selecting
/// it if it does not.
///
/// Gap: a binding computed to feed another unit must be computed from the
/// same values that unit will read. Those need not be a published State: the
/// binding may be needed before anything is published, or for a State
/// several unpublished updates on. How a selection states which values it is
/// computed from is not written yet.
///
/// Decide: how the runtime knows a held binding is still good for the State it
/// is about to read. The simplest rule is that a binding's value stays valid
/// while every value it was computed from is unchanged.
///
/// Decide: whether a supplier may be asked for a unit while earlier units from
/// it are running. The runtime needs this for a selection made after launch.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    /// One unit that computes every root selected, for one target.
    fn unit(&self, roots: &[Root], target: Target) -> Result<Emitted, Self::Diagnostic>;
}
