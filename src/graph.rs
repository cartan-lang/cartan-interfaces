//! Source meaning: names, types, dependencies and the normalized computation.

use crate::program::Program;
use crate::supply::Request;
use crate::syntax::SyntaxTree;

/// A resolved, typed program. Documents and libraries are supplied by the
/// caller; Graph reads no files.
///
/// Gap: this is a placeholder. What Codegen and Compile may ask of an
/// analysis is not written yet.
pub struct Analysis {
    _private: (),
}

/// How one Array of a computation is held.
///
/// An Array is a domain and a rule giving the fiber at each site. Its kind
/// does not say whether its elements are kept in memory. That is a fusion
/// decision, made for each Array of a computation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backing {
    /// The elements are computed once into storage and read from there.
    Stored,
    /// The elements are computed wherever they are read. Nothing is stored.
    Lazy,
}

/// One fusion plan for one request: which values the computation takes as
/// inputs, and the backing of every Array inside it.
///
/// Gap: this is a placeholder. How a plan is described and compared is not
/// written yet.
pub struct Plan {
    _private: (),
}

/// A normalized computation under one fusion plan.
///
/// The same type whether the request is a launch, an update or an observation
/// made later. One pipeline serves all three; the request selects roots and
/// does not select a different compiler.
///
/// Every Array in it is a domain and a rule for the fiber at each site, with
/// the backing its plan chose. A lazy Array's rule is fully known when the
/// computation is lowered and is evaluated in place wherever the Array is
/// read; it is not a value, is never stored and never leaves the
/// computation. An Array that is unbounded on some axis can only be lazy.
/// Codegen lowers the backing it is given and does not choose it.
///
/// A computation's edge is the same decision one step out. An upstream
/// binding is either an input, computed once and held by the runtime, or
/// part of the computation. Every input and every result is stored.
///
/// No Array is sized from its own contents. A domain may depend on data, but
/// only on values computed before the Array is filled, such as a site of
/// Ints held by an upstream Array.
///
/// Gap: this is a placeholder. The representation Codegen lowers from, and
/// how a domain and an element rule are written in it, are not written yet.
pub struct Computation {
    _private: (),
}

/// Graph's offer of fusion plans for one request.
///
/// Graph does not hand over one computation. It offers the plans it can
/// produce and lets whoever chooses steer among them. A fixed sequence of
/// candidate plans is the simplest navigator.
///
/// Decide: who steers, and whether the choice may depend on what the runtime
/// already holds. Either way a prepared unit's inputs never change; a
/// different plan is a different unit. A binding that depends on no State an
/// update writes is a candidate for an input, since it is the same on every
/// step, but not an obligation: a cheap one can stay inline.
///
/// Gap: how a navigator is steered from one plan to another is not written
/// yet.
pub trait Navigate {
    type Diagnostic;

    /// The plan Graph proposes when nobody steers.
    fn start(&self) -> Plan;

    /// The computation one plan describes.
    fn computation(&self, plan: &Plan) -> Result<Computation, Self::Diagnostic>;
}

/// Resolve a source, then offer the fusion plans for what a request needs.
pub trait Analyze {
    type Diagnostic;
    type Navigator: Navigate<Diagnostic = Self::Diagnostic>;

    fn analyze(
        &self,
        root: &SyntaxTree,
        libraries: &[(&str, &SyntaxTree)],
    ) -> Result<Analysis, Self::Diagnostic>;

    fn program<'a>(&self, analysis: &'a Analysis) -> &'a Program;

    fn plans(
        &self,
        analysis: &Analysis,
        request: Request<'_>,
    ) -> Result<Self::Navigator, Self::Diagnostic>;
}
