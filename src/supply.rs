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
/// Every value is the value of something at one definite version. A unit is
/// compiled once and says nothing about versions: the runtime runs it for a
/// version by binding its inputs to the values of that version, and the
/// results are the values at that version. A binding computed to feed another
/// unit is therefore computed for the version that unit reads. Version zero
/// exists before any update, since every State has an initial value, so a
/// binding can be computed before anything else has happened.
///
/// A unit's results depend on nothing but its inputs. A value computed at
/// one version is the value at every later version at which nothing it
/// depends on has changed, where what it depends on runs through any
/// bindings down to States and Params. The runtime may keep a computed value
/// and use it again on that basis; once computed, it never has to be
/// computed again to a different result. How the runtime notices that
/// something has changed is its own business.
///
/// A selection may be made at any time, including while units the supplier
/// produced earlier are running. Producing a new unit never disturbs code
/// already running or values already computed. The runtime owes values for
/// the latest version and may skip computing them for versions in between.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    /// One unit that computes every root selected, for one target.
    fn unit(&self, roots: &[Root], target: Target) -> Result<Emitted, Self::Diagnostic>;
}
