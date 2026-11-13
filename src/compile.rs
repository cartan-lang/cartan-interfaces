//! Immutable preparation: emitted text or bytes become loadable code.

use crate::supply::Emitted;
use crate::unit::{Checked, Unit};

/// Prepared code with the checked unit description passed through untouched.
///
/// It is immutable and reusable: the same prepared code can be loaded for
/// several devices of one architecture and run any number of times.
pub struct Prepared<C> {
    unit: Checked,
    code: C,
}

impl<C> Prepared<C> {
    pub fn new(unit: Checked, code: C) -> Self {
        Self { unit, code }
    }

    pub fn unit(&self) -> &Unit {
        self.unit.unit()
    }

    pub fn code(&self) -> &C {
        &self.code
    }
}

/// Turn emitted text or bytes into loadable code inside an existing backend
/// context, so a request made after launch joins the running program. It
/// accepts any supplier's output.
pub trait Prepare {
    type Context;
    type Code;
    type Diagnostic;

    fn prepare(
        &self,
        emitted: Emitted,
        context: &Self::Context,
    ) -> Result<Prepared<Self::Code>, Self::Diagnostic>;
}
