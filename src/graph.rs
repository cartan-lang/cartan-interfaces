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

/// How one indexed value of a computation is held.
///
/// An indexed value is a domain and a rule giving the value at each site. Its
/// kind does not say whether its values are kept in memory. That is a fusion
/// decision.
///
/// These two are not required to be the only backings. A value read from
/// storage at some sites and computed by its rule at the others is neither
/// designed here nor excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Backing {
    /// The values are computed once into storage and read from there.
    Stored,
    /// The values are computed wherever they are read. Nothing is stored.
    Lazy,
}

/// One indexed value of a computation that Graph offers a choice of backing
/// for.
///
/// An Array the source names and an intermediate that only exists while a
/// comprehension body is evaluated at one index lower the same way: each is a
/// value at an index. Graph decides which of them are eligible to be held in
/// storage. It does not offer every temporary, and it does not leave one out
/// only because the source gave it no name.
///
/// Gap: how Graph finds candidates, and what identifies one beyond its place
/// in this list, is not written yet.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// What to call it when showing a plan to a person.
    pub label: String,
    /// The one backing it can have, when there is no choice. A result is
    /// always stored. A value unbounded on some axis is never stored whole.
    pub forced: Option<Backing>,
}

/// One fusion plan for one request: a backing for each candidate, in the
/// order the navigator lists them.
///
/// A plan is a complete selection and can be written down and asked for
/// directly, for example as one character per candidate. With two backings
/// to choose from, the plans for `n` candidates are the corners of an
/// `n`-cube. That is a way to name plans. It does not mean every corner is
/// admissible, or that changing one candidate at a time reaches every
/// admissible plan.
///
/// Decide: the order of candidates is fixed for one analyzed request. Whether
/// a written plan still means the same thing after the source changes is not
/// settled, so a plan is not yet something to save and replay.
///
/// Gap: a stored value may be computed inside the computation or taken as an
/// input from outside it. A plan does not yet say which.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub backing: Vec<Backing>,
}

/// A normalized computation under one fusion plan.
///
/// The same type whether the request is a launch, an update or an observation
/// made later. One pipeline serves all three; the request selects roots and
/// does not select a different compiler.
///
/// An indexed computation supplies values at requested sites. Every indexed
/// value in it is a domain and a rule for the value at each site, with the
/// backing its plan chose. A rule is ordinary scoped computation: it may read
/// other values at fixed offsets, at computed indices, or across a whole
/// domain inside a reduction. A rule that is not stored is fully known when
/// the computation is lowered and is evaluated in place wherever the value is
/// read; it is not a value of the language and never leaves the computation.
/// Codegen lowers the backing it is given and does not choose it.
///
/// A computation's edge is the same decision one step out. An upstream
/// binding is either an input, computed once and held by the runtime, or
/// part of the computation. Every input and every result is stored. That
/// does not require a fresh copy: a result may share storage that already
/// exists and will not change.
///
/// No indexed value is sized from its own contents. A domain may depend on
/// data, but only on values computed before it is filled, such as a site of
/// Ints held by an upstream Array.
///
/// Gap: an Array unbounded on some axis cannot be stored whole, yet every
/// result is stored. Whether a finite window of one is a separate finite
/// result or a partly stored form of the original is not settled.
///
/// Whatever the representation becomes, it states shared structure once. A
/// value used in several places, a rule read at several offsets and an
/// expression that appears in both branches of a conditional each appear once
/// and are referred to. Nothing in it may require expanding a shared thing at
/// every use, or prevent either side from hoisting it.
///
/// Gap: this is a placeholder. The representation Codegen lowers from, and
/// how a domain and a rule are written in it, are not written yet.
pub struct Computation {
    _private: (),
}

/// Graph's offer of fusion plans for one request.
///
/// Graph does not hand over one computation. It lists the candidates it
/// exposes and turns any plan over them into a computation, or says why that
/// plan cannot be had. A plan chooses backing only for those candidates.
/// Choosing by hand and searching systematically are the same interface with
/// a different chooser.
///
/// Decide: who chooses, and whether the choice may depend on what the runtime
/// already holds. Either way a prepared unit's inputs never change; a
/// different plan is a different unit. A binding whose value does not change
/// while everything it was computed from is unchanged is a candidate for an
/// input, but not an obligation: a cheap one can stay inline. Depending on no
/// State that one update writes is not enough to make it constant, since a
/// Param or another update can change what it reads.
pub trait Navigate {
    type Diagnostic;

    /// The candidates, in the order a plan lists their backings.
    fn candidates(&self) -> &[Candidate];

    /// The plan Graph proposes when nobody chooses.
    fn start(&self) -> Plan;

    /// The computation one plan describes, or why that plan is not available.
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
