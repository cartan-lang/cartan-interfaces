//! Every official term of these contracts, in one place.
//!
//! A term that is a type is defined once, in the documentation of that type,
//! and is listed here by re-export. A term that is a field, a variant or a
//! method is defined where it is declared, and is listed here by a link. A
//! term that is none of these is defined here and nowhere else.
//!
//! No other document defines these terms. A word the contracts need that is
//! not here is added here first.
//!
//! # Terms that are fields, variants or methods
//!
//! | Term | Declared in |
//! |---|---|
//! | Presence, length, domain | [`ShapeValue`] |
//! | Specialization | [`ShapeEntry::Fixed`] |
//! | Element size | [`Need::element_bytes`] |
//! | Alignment | [`Need::align`] |
//! | Base, offset | [`Region`] |
//! | Input storage, output storage, scratch, alias | [`Storage`] |
//! | Port | [`InputPort`], [`OutputPort`] |
//! | Host, linear memory, device | [`Memory`] |
//! | Uses, determines | [`Stage::uses`], [`Stage::determines`] |
//! | Fallback | [`Selection::otherwise`] |
//! | State, Param, derived value | [`Node`] |
//! | Initial value, next value | [`Demand`] |
//! | Fiber | [`Definition`] |
//! | Shortage, cancellation | [`Stop`] |
//! | Run | [`Session`] |
//! | Retained value | [`Session::Value`] |
//! | Absent, empty | [`ShapeValue::is_absent`], [`ShapeValue::is_empty`] |
//! | Governs | [`Checked::governing`] |
//! | List order | [`Unit::stages`] |
//!
//! # Terms that are not Rust items
//!
//! Each of these stays a word for one of four reasons.
//!
//! - It names something that happens inside a runtime and crosses no
//!   boundary: runner, handshake, sync point, submission and completion,
//!   readback.
//! - It names something a description never states: allocation size.
//! - It names a rule or a condition and not a thing: applies, shape before
//!   fill, capacity bound, fit. A rule shows up as a check and its defects.
//! - It waits on a part of a contract that is not written yet: launch, which
//!   becomes a type with the calling convention of a stage, and site, which
//!   becomes one when the sites asked for can be stated.
//!
//! ## Launch
//!
//! The runtime starting one [`Stage`]. A stage is one launch.
//!
//! ## Runner
//!
//! The part of a runtime that drives one run and launches its stages, on
//! whatever threads and devices the run uses. How it is built is the
//! runtime's own business.
//!
//! ## Handshake
//!
//! The blocking round trip in which a runtime waits for one [`Update`] to
//! finish and accepts it before it starts the next. The word has no other
//! meaning.
//!
//! ## Sync point
//!
//! A moment at which the host must wait for work on a device to complete.
//!
//! ## Submission and completion
//!
//! Work handed to a device has been submitted. It has completed when the
//! device has finished it. The host can read what a stage produced on a
//! device only after completion.
//!
//! ## Readback
//!
//! The host reading something a device produced.
//!
//! ## Site
//!
//! One position of an indexed value: an index of a List or a point of the
//! domain of an Array.
//!
//! ## Allocation size
//!
//! The bytes of an allocation. No description states one. A runtime works it
//! out from an element size and a [`Capacity`].
//!
//! ## Applies
//!
//! A shape value applies when the value it describes exists. Those within an
//! Optional that is absent do not apply. See [`ShapeEntry`].
//!
//! ## Shape before fill
//!
//! No value takes its shape from its own contents. One stage determines a
//! shape value and a later stage fills what it governs. See [`Stage`].
//!
//! ## Capacity bound
//!
//! A [`Capacity`] given by a shape value produced before the result's own,
//! such as the length of an input for a result that can be no longer.
//!
//! ## Fit
//!
//! What code writes into a slot fits when it stays within the slot's
//! [`Capacity`].
//!
//! ## Gap and Decide
//!
//! A doc paragraph starting `Gap:` names a part of a contract that is not
//! written yet. One starting `Decide:` names an open choice.

pub use crate::kind::{AxisKind, Definition, KindDefect, KindRef, Kinds, Primitive, ShapeField};
pub use crate::layout::{Layout, LayoutDefect, LayoutRef, Layouts, Measures, Part, Plane, Scalar};
pub use crate::program::{
    Declared, Demand, DerivedId, Node, ParamId, Program, StateId, Update, UpdateId,
};
pub use crate::session::{Session, Stop, Version};
pub use crate::supply::{Emitted, Supplier};
pub use crate::unit::{
    Axis, Capacity, Checked, Code, Consumer, Defect, InputPort, Memory, Need, OutputPort, PortRef,
    Producer, Region, Selection, ShapeEntry, ShapeRef, ShapeValue, Slot, Stage, Storage, Target,
    Unit,
};

#[cfg(feature = "pipeline")]
pub use crate::codegen::Emit;
#[cfg(feature = "pipeline")]
pub use crate::compile::{Prepare, Prepared};
#[cfg(feature = "pipeline")]
pub use crate::graph::{Analysis, Analyze, Backing, Candidate, Computation, Navigate, Plan};
#[cfg(feature = "pipeline")]
pub use crate::syntax::{Parse, SyntaxTree};
