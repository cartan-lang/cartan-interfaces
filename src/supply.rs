//! What a compiler front half supplies to the runtime.

use crate::program::{Demand, Program};
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
/// The runtime demands what is to be computed and the supplier compiles it.
/// Every demand is met by the same kind of computation: read values, produce
/// values. What the runtime then does with the results is its own business
/// and no part of the demand. It makes the first State from every State's
/// initial value. It commits an update by taking the new values of all the
/// States the block writes together. It reports an observation from the
/// values of the nodes it was asked for.
///
/// An input port may be a derived value. The supplier chooses, value by
/// value, between computing it inside the unit and having it computed once
/// and passed in. This is the same kind of choice as fusing or separating the
/// steps within a unit, made at the unit's edge. When a unit lists a derived
/// value as an input, the runtime must hold that value first, demanding it if
/// it does not. The one exception is an input a unit uses only as the
/// fallback of a selection: it is needed only when it would be chosen.
/// Because a derived value is identified by what it computes, a value one
/// unit produces and another takes in is the same node in both, whether or
/// not the source names it.
///
/// Every value is the value of something at one definite version. A unit is
/// compiled once and says nothing about versions: the runtime runs it for a
/// version by binding its inputs to the values of that version, and the
/// results are the values at that version. A derived value computed to feed another
/// unit is therefore computed for the version that unit reads. Version zero
/// exists before any update, since every State has an initial value, so a
/// derived value can be computed before anything else has happened.
///
/// A unit's results depend on nothing but its inputs. A value computed at
/// one version is the value at every later version at which nothing it
/// depends on has changed, where what it depends on runs through any
/// derived values down to States and Params. The runtime may keep a computed value
/// and use it again on that basis; once computed, it never has to be
/// computed again to a different result. How the runtime notices that
/// something has changed is its own business.
///
/// A demand may be made at any time, including while units the supplier
/// produced earlier are running. Producing a new unit never disturbs code
/// already running or values already computed. The runtime owes values for
/// the latest version and may skip computing them for versions in between.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    /// One unit that meets every demand, for one target.
    fn unit(&self, demands: &[Demand], target: Target) -> Result<Emitted, Self::Diagnostic>;
}
