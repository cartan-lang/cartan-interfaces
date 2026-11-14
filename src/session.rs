//! What an application asks of the runtime, and what the runtime reports back.
//!
//! The runtime owns a run and its storage. A run may complete many atomic
//! updates between publications: each update produces the next State, but a
//! State becomes visible to anything outside the run only when it is
//! published. Compiled computations supply the transitions and the decisions
//! about cadence and stopping. Publication exposes one immutable, complete
//! State with its version and the Params it was computed with. A required
//! observation may hold back advancement until it is taken; optional
//! rendering may not. Where the repetition of updates executes is not
//! specified.

use crate::program::Node;
use crate::unit::Memory;

/// One State of a running program. Each completed update makes the next,
/// whether or not it is published.
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
    Binding(Node),
    /// The unit names a memory this runtime has not set up.
    Unplaced(Memory),
    /// The code did not do what its own description says, such as never
    /// producing a shape value it determines. This is a defect in a code
    /// generator, not a run-time outcome of the program.
    Invariant,
    /// A device or its driver failed.
    Device,
}

/// One run: one execution of a program by a runtime, from its first State
/// onward, as an application drives it.
///
/// A session coordinates a supplier and the runtime. It is not another
/// evaluator, and nothing outside the runtime writes State.
///
/// Gap: this is a placeholder and its methods are provisional. Starting a run,
/// parameters, how publication is requested or offered, delivery of results
/// and stopping conditions are not written yet. `advance` states a number of
/// updates, not how they are run or which update block runs when there are
/// several. `observe` does not say which State and Params its values belong
/// to. Both report with `Stop`, which is why a run ended; a request that is
/// refused, or that cannot be compiled, need not end the run and has no
/// result type of its own yet.
pub trait Session {
    /// A retained value: one the runtime keeps after the stages that
    /// produced it have finished, with its shape values. Reading its bytes
    /// back is a separate, explicit step.
    type Value;

    /// Complete this many updates and report the version reached.
    ///
    /// Asking for several updates at once is what lets a runtime avoid a
    /// [handshake](crate::glossary#handshake) on every update.
    fn advance(&mut self, updates: u64) -> Result<Version, Stop>;

    fn observe(&mut self, nodes: &[Node]) -> Result<Vec<(Node, Self::Value)>, Stop>;
}
