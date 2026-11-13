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

/// A normalized, fused computation for one request.
///
/// The same type whether the request is a launch, an update or an observation
/// made later. One pipeline serves all three; the request selects roots and
/// does not select a different compiler.
///
/// Its boundary is a fusion decision, and Graph makes it. Cutting an
/// expression inside the computation gives a private intermediate. Cutting at
/// the edge gives an upstream binding the computation takes as an input,
/// computed once and held by the runtime. Everything not cut is evaluated
/// inline. Codegen lowers the inputs it is given and does not choose them.
///
/// Decide: whether the choice may depend on what the runtime already holds.
/// Made from the source alone, a prepared unit keeps the same inputs for its
/// whole life. A binding that does not depend on any State an update writes is
/// the natural input: it is the same on every step.
///
/// Gap: this is a placeholder. The representation Codegen lowers from is not
/// written yet.
pub struct Computation {
    _private: (),
}

/// Resolve a source, then select and normalize what a request needs.
pub trait Analyze {
    type Diagnostic;

    fn analyze(
        &self,
        root: &SyntaxTree,
        libraries: &[(&str, &SyntaxTree)],
    ) -> Result<Analysis, Self::Diagnostic>;

    fn program<'a>(&self, analysis: &'a Analysis) -> &'a Program;

    fn computation(
        &self,
        analysis: &Analysis,
        request: Request<'_>,
    ) -> Result<Computation, Self::Diagnostic>;
}
