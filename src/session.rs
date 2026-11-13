//! What an application asks of the runtime, and what the runtime reports back.

use crate::program::Name;
use crate::unit::Memory;

/// One committed State of a running program. Each accepted update makes the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64);

/// Why a run ended without a result. None of these publish anything.
///
/// None is a failure of the program. Cartan is total: compiled code always
/// completes, and an error-looking result is an ordinary value. Every stop is
/// something the runtime discovers.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The runtime could not provide storage within the memory limit. It
    /// finds this out because it handles every allocation.
    Shortage,
    Cancelled,
    /// An input was missing or did not match the unit's declared port.
    Binding(Name),
    /// The unit asks for a memory this runtime has not set up.
    Unplaced(Memory),
    /// The code did not do what its own description says, such as leaving a
    /// size unstated. This is a defect in a code generator, not a run-time
    /// outcome of the program.
    Invariant,
    /// A device or its driver failed.
    Device,
}

/// One running program, as an application drives it.
///
/// A session coordinates a supplier and the runtime. It is not another
/// evaluator, and nothing outside the runtime writes State.
///
/// Gap: this is a placeholder. Launching, parameters, cadenced observation,
/// delivery of results and stopping conditions are not written yet.
pub trait Session {
    /// A computed value. Reading its bytes back is a separate, explicit step.
    type Value;

    fn advance(&mut self, updates: u64) -> Result<Version, Stop>;

    fn observe(&mut self, names: &[Name]) -> Result<Vec<(Name, Self::Value)>, Stop>;
}
