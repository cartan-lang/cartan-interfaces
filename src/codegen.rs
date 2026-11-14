//! Target emission: a computation becomes code and its unit description.

use crate::graph::Computation;
use crate::supply::Emitted;
use crate::unit::Target;

/// Lower one computation for one target, describing the result as one `Unit`.
///
/// Codegen chooses the physical representation: how each scalar is stored,
/// which planes a value uses and where each part sits in them. It does not choose what the computation contains or which
/// values are its inputs.
///
/// Gap: a value is bound to a port only if their layouts are identical, yet
/// nothing here tells Codegen the layout of a value that already exists, or
/// asks for a conversion. Two units can agree on a kind and still disagree on
/// its layout.
pub trait Emit {
    type Diagnostic;

    fn emit(&self, computation: &Computation, target: Target) -> Result<Emitted, Self::Diagnostic>;
}
