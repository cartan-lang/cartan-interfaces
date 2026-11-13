//! The description of one piece of compiled code.

use crate::kind::{Dimension, Kind, Primitive};
use crate::program::Name;

/// One axis of an Array domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub origin: i64,
    pub extent: u64,
}

/// What a dimension holds once it is known: a size, or presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Size {
    /// Whether an Optional value is present. Not a size; it counts nothing.
    Presence(bool),
    Length(u64),
    Domain(Vec<Axis>),
}

impl Size {
    /// How many elements this size describes, or nothing if it is presence
    /// or the count does not fit in 64 bits.
    pub fn count(&self) -> Option<u64> {
        match self {
            Size::Presence(_) => None,
            Size::Length(length) => Some(*length),
            Size::Domain(axes) => axes
                .iter()
                .try_fold(1u64, |count, axis| count.checked_mul(axis.extent)),
        }
    }

    pub fn is(&self, dimension: Dimension) -> bool {
        match (self, dimension) {
            (Size::Presence(_), Dimension::Presence) => true,
            (Size::Length(_), Dimension::Length) => true,
            (Size::Domain(axes), Dimension::Domain { rank }) => axes.len() == rank as usize,
            _ => false,
        }
    }
}

/// Position of one dimension in a unit's shape record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dim(pub u32);

/// A size or presence that is either fixed in the code or found in the shape
/// record.
///
/// The shape record is one table per run of a unit, with one entry for each of
/// the unit's dimensions. The runtime fills the entries that describe inputs.
/// The unit's stages fill the rest. That is how a unit reports the sizes of
/// what it produced and whether an Optional result is present, and how the
/// runtime learns either without reading the value's storage.
///
/// An entry that has not been filled is different from one that says a value
/// is absent. The runtime knows the presence of an input before the unit
/// runs. It knows the presence of a result only once the stage that
/// determines it has returned, so storage it provided beforehand may go
/// unused. The sizes of an absent value are left unfilled.
///
/// In an input port, `Fixed` is a specialization the runtime verifies against
/// the actual value before running. A change of extent needs no recompilation
/// unless the code chose to specialize on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sizing {
    Fixed(Size),
    Runtime(Dim),
}

/// Where a piece of storage lives.
///
/// Every slot names its memory, so one unit can keep small host-side values
/// beside buffers on one or more devices.
///
/// Decide: a device is numbered within the unit, and the runtime chooses which
/// physical device each number means. Whether a description may instead name
/// a physical device is not settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Memory {
    /// Ordinary memory of the host process.
    Host,
    /// The linear memory of the Wasm instance that runs the unit.
    Linear,
    /// The memory of one compute device, numbered within the unit.
    Device(u32),
}

/// A machine scalar: how one leaf of a value is stored.
///
/// Decide: an Int is stored as `I32` or `I64` and a Real as `F32` or `F64`.
/// How a Bool is stored is not settled, so any scalar is accepted for one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    I32,
    I64,
    F32,
    F64,
}

impl Scalar {
    /// Whether this scalar can store the given primitive.
    pub fn stores(self, primitive: Primitive) -> bool {
        match primitive {
            Primitive::Bool => true,
            Primitive::Int => matches!(self, Scalar::I32 | Scalar::I64),
            Primitive::Real => matches!(self, Scalar::F32 | Scalar::F64),
        }
    }

    pub fn bytes(self) -> u32 {
        match self {
            Scalar::I32 | Scalar::F32 => 4,
            Scalar::I64 | Scalar::F64 => 8,
        }
    }
}

/// Where one scalar leaf of a value sits: which of the value's planes, and at
/// what byte offset within each element of that plane.
///
/// This is what makes layout a choice. Give every leaf its own plane and an
/// Array of fibers is stored as separate arrays, one per component. Put every
/// leaf in one plane at different offsets and it is stored as one array of
/// whole fibers. Anything between is also expressible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leaf {
    pub scalar: Scalar,
    /// Index into the port's `planes`.
    pub plane: u32,
    pub offset: u32,
}

/// One entry in a unit's storage table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(pub u32);

/// What storage a slot needs: which memory, and how much of it as a
/// compile-time element size times a run-time capacity.
///
/// A unit never asks for a number of bytes, and it never asks at all: this
/// description is the request. The runtime reads it, with the sizes in the
/// shape record, and provides the storage before the stage that writes it.
#[derive(Clone, Debug)]
pub struct Need {
    pub memory: Memory,
    pub element_bytes: u32,
    pub align: u32,
    /// How many elements the slot must be able to hold: the product of these.
    /// Empty means one element. Every size here is known before the stage
    /// that first writes the slot. The value written may be smaller than the
    /// capacity; its actual sizes are the ones in its port.
    pub capacity: Vec<Sizing>,
}

/// What each slot is for. This decides who may write it.
#[derive(Clone, Debug)]
pub enum Storage {
    /// A plane of an input value: the memory, element size and number of
    /// elements the unit expects to find. Never written.
    Input(Need),
    /// A plane of a result. Written by this unit, then handed to the runtime.
    Output(Need),
    /// A result plane that is an input plane unchanged. Shares its storage.
    ///
    /// Decide: an alias names one fixed input. A result that is one of two
    /// inputs depending on a run-time fact, such as an Optional State when
    /// present and its initial value otherwise, cannot be expressed without
    /// copying.
    Alias(Slot),
    /// Private to one run of the unit. It has no identity outside it.
    Scratch(Need),
}

/// One value entering or leaving a unit: its kind, its sizes and its physical
/// layout. The kind says what the value is. The sizes and leaves must match
/// it: one size for each of the kind's dimensions and one leaf for each of
/// its scalars, in the kind's order.
///
/// Decide: a value is bound to a port only if their layouts are identical.
/// Converting between layouts would be an explicit computation, never
/// something the runtime does silently while binding.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: Name,
    pub kind: Kind,
    /// One entry per dimension of the kind, in the kind's order.
    pub shape: Vec<Sizing>,
    /// One entry per scalar of the kind, in the kind's order.
    pub leaves: Vec<Leaf>,
    /// The slots that hold the value. Each is one physical plane.
    pub planes: Vec<Slot>,
}

/// One entry point in the code. Stages run in order.
///
/// Compiled code never calls the runtime. Storage is passed in and sizes are
/// reported out. A stage receives the storage it reads and writes and the
/// shape record, computes, writes the sizes it determines into the record and
/// returns. All providing of storage, and any waiting for it, happens in the
/// runtime between stages.
///
/// A result whose size depends on data the unit computes is handled in one of
/// two ways. A stage can be given storage with capacity for a bound known
/// beforehand and report the actual size it produced. Or one stage can
/// measure and a later stage fill, with the runtime providing storage between
/// them.
///
/// A stage has no failure to report. Cartan is total, so compiled code always
/// runs to completion and every outcome it computes is an ordinary value. A
/// run can still end early, but only for reasons the runtime discovers, such
/// as being unable to provide storage or being cancelled, and only between
/// stages.
///
/// A stage may do any amount of work, including several updates, as long as it
/// needs nothing from the runtime while it runs. Where repetition happens is
/// not part of this contract.
///
/// Gap: the calling convention of a stage on each target is not written yet:
/// how the entry is named, how slots and the shape record are passed, and the
/// extent a parallel stage is dispatched over.
#[derive(Clone, Debug)]
pub struct Stage {
    pub reads: Vec<Slot>,
    pub writes: Vec<Slot>,
    /// Dimensions this stage writes into the shape record.
    pub determines: Vec<Dim>,
}

/// Everything the runtime needs to know to run a piece of compiled code.
///
/// There is one kind of unit. A scalar result, an Array fill, a multi-stage
/// update and a fused kernel differ only in how many ports, slots and stages
/// they list. A unit asked for by an update and a unit asked for by an
/// observation are also the same kind of unit: a computation that reads named
/// values and produces named values. Whether its results become the next State
/// is decided by the request, not by anything in the unit. Producing the value
/// of a State is not the same as writing it.
#[derive(Clone, Debug, Default)]
pub struct Unit {
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub storage: Vec<Storage>,
    /// The layout of the shape record: every size not fixed at compile time.
    pub dimensions: Vec<Dimension>,
    pub stages: Vec<Stage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Bytecode,
    Wasm,
    Metal,
    Cuda,
}

/// Target code as text or bytes. The runtime's side prepares and loads it.
///
/// Gap: the bytecode format is the official code generator's own and has no
/// public specification yet.
pub enum Code {
    Bytecode(Vec<u32>),
    Wasm(Vec<u8>),
    Metal(String),
    Cuda(String),
}

/// Why a unit description is not well formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    UnknownSlot(Slot),
    UnknownDimension(Dim),
    /// A stage writes storage whose capacity is settled by no input and no
    /// earlier stage. Storage is provided before a stage runs, so a stage
    /// cannot settle the capacity of what it writes.
    SizeNotYetKnown {
        stage: u32,
        dim: Dim,
    },
    /// A result's size is never determined.
    ResultSizeUnknown(Dim),
    /// An input port names a slot that is not `Storage::Input`.
    InputPlaneNotInput(Slot),
    /// An output port names a slot that is neither `Output` nor `Alias`.
    OutputPlaneNotResult(Slot),
    /// An alias must refer to an input slot.
    AliasOfNonInput(Slot),
    /// A stage writes a slot it does not own.
    WritesShared {
        stage: u32,
        slot: Slot,
    },
    /// A port's sizes or leaves do not match its kind: the wrong number of
    /// either, a size of the wrong sort, or a scalar that cannot store its
    /// primitive.
    KindMismatch(Name),
    /// A port's kind holds an Optional directly inside an Optional.
    NestedOptional(Name),
    /// A storage capacity is stated as presence, which is not a size.
    CapacityNotASize(Slot),
    /// A port's kind includes a fiber whose storage this contract does not
    /// describe yet. The kind is valid; a unit cannot carry it across.
    KindUnsupported(Name),
    /// A leaf names a plane its port does not have.
    UnknownPlane {
        leaf: u32,
    },
    /// A leaf does not fit inside one element of its plane.
    LeafOutsideElement {
        leaf: u32,
    },
    /// Two leaves of one value occupy the same bytes.
    LeavesOverlap {
        first: u32,
        second: u32,
    },
}

/// A unit description that passed [`Unit::check`].
///
/// The field is private, so holding one is proof the description passed these
/// checks: every slot and dimension it names exists; each port's kind is one
/// whose storage is described and which nests no Optional, and its sizes,
/// presence and leaves match it; no capacity is stated as presence; no stage writes an input
/// or an alias; an alias refers to an input; the capacity of storage is known
/// before the stage that writes it; every leaf fits inside one element of its plane and
/// overlaps no other. It does not check that every result is written by some
/// stage, and it cannot show that the code honors the description.
///
/// Decide: how far the runtime trusts code from a supplier it did not build.
/// Nothing in this contract confines code to the storage it declared.
#[derive(Clone, Debug)]
pub struct Checked(Unit);

impl Checked {
    pub fn unit(&self) -> &Unit {
        &self.0
    }
}

impl Unit {
    /// Check that the description is well formed.
    pub fn check(self) -> Result<Checked, Defect> {
        let storage = |slot: Slot| {
            self.storage
                .get(slot.0 as usize)
                .ok_or(Defect::UnknownSlot(slot))
        };
        let dim = |sizing: &Sizing| match sizing {
            Sizing::Runtime(dim) if dim.0 as usize >= self.dimensions.len() => {
                Err(Defect::UnknownDimension(*dim))
            }
            _ => Ok(()),
        };
        for (index, entry) in self.storage.iter().enumerate() {
            match entry {
                Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need) => {
                    for sizing in &need.capacity {
                        dim(sizing)?;
                        let presence = match sizing {
                            Sizing::Fixed(size) => matches!(size, Size::Presence(_)),
                            Sizing::Runtime(dim) => {
                                self.dimensions[dim.0 as usize] == Dimension::Presence
                            }
                        };
                        if presence {
                            return Err(Defect::CapacityNotASize(Slot(index as u32)));
                        }
                    }
                }
                Storage::Alias(of) => {
                    if !matches!(storage(*of)?, Storage::Input(_)) {
                        return Err(Defect::AliasOfNonInput(Slot(index as u32)));
                    }
                }
            }
        }

        // A port's sizes and leaves are the ones its kind calls for.
        for port in self.inputs.iter().chain(&self.outputs) {
            let mismatch = Defect::KindMismatch(port.name);
            if port.kind.nests_optional() {
                return Err(Defect::NestedOptional(port.name));
            }
            let Some(count) = port.kind.leaf_count() else {
                return Err(Defect::KindUnsupported(port.name));
            };
            let dimensions = port.kind.dimensions();
            if dimensions.len() != port.shape.len() || count != port.leaves.len() as u64 {
                return Err(mismatch);
            }
            let Some(primitives) = port.kind.leaves() else {
                return Err(Defect::KindUnsupported(port.name));
            };
            for (dimension, sizing) in dimensions.iter().zip(&port.shape) {
                let fits = match sizing {
                    Sizing::Fixed(size) => size.is(*dimension),
                    Sizing::Runtime(dim) => self.dimensions.get(dim.0 as usize) == Some(dimension),
                };
                if !fits {
                    return Err(mismatch);
                }
            }
            for (primitive, leaf) in primitives.iter().zip(&port.leaves) {
                if !leaf.scalar.stores(*primitive) {
                    return Err(mismatch);
                }
            }
        }

        // Every leaf sits inside one element of a plane, overlapping no other.
        for port in self.inputs.iter().chain(&self.outputs) {
            let mut placed: Vec<(u32, u32, u32)> = Vec::new();
            for (index, leaf) in port.leaves.iter().enumerate() {
                let index = index as u32;
                let slot = port
                    .planes
                    .get(leaf.plane as usize)
                    .ok_or(Defect::UnknownPlane { leaf: index })?;
                let element = match storage(*slot)? {
                    Storage::Alias(of) => storage(*of)?,
                    other => other,
                };
                let (Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need)) =
                    element
                else {
                    return Err(Defect::AliasOfNonInput(*slot));
                };
                let end = match leaf.offset.checked_add(leaf.scalar.bytes()) {
                    Some(end) if end <= need.element_bytes => end,
                    _ => return Err(Defect::LeafOutsideElement { leaf: index }),
                };
                for (other, start, stop) in &placed {
                    if *other == leaf.plane && leaf.offset < *stop && *start < end {
                        let first = placed.iter().position(|p| p == &(*other, *start, *stop));
                        return Err(Defect::LeavesOverlap {
                            first: first.unwrap_or(0) as u32,
                            second: index,
                        });
                    }
                }
                placed.push((leaf.plane, leaf.offset, end));
            }
        }

        // A size is known once an input supplies it or a stage determines it.
        let mut known = vec![false; self.dimensions.len()];
        for port in &self.inputs {
            for sizing in &port.shape {
                dim(sizing)?;
                if let Sizing::Runtime(dim) = sizing {
                    known[dim.0 as usize] = true;
                }
            }
            for slot in &port.planes {
                if !matches!(storage(*slot)?, Storage::Input(_)) {
                    return Err(Defect::InputPlaneNotInput(*slot));
                }
            }
        }
        let mut provided = vec![false; self.storage.len()];
        for (index, stage) in self.stages.iter().enumerate() {
            for slot in &stage.reads {
                storage(*slot)?;
            }
            for slot in &stage.writes {
                let (Storage::Output(need) | Storage::Scratch(need)) = storage(*slot)? else {
                    return Err(Defect::WritesShared {
                        stage: index as u32,
                        slot: *slot,
                    });
                };
                if !std::mem::replace(&mut provided[slot.0 as usize], true) {
                    for sizing in &need.capacity {
                        if let Sizing::Runtime(dim) = sizing {
                            if !known[dim.0 as usize] {
                                return Err(Defect::SizeNotYetKnown {
                                    stage: index as u32,
                                    dim: *dim,
                                });
                            }
                        }
                    }
                }
            }
            for determined in &stage.determines {
                *known
                    .get_mut(determined.0 as usize)
                    .ok_or(Defect::UnknownDimension(*determined))? = true;
            }
        }
        for port in &self.outputs {
            for sizing in &port.shape {
                dim(sizing)?;
                if let Sizing::Runtime(dim) = sizing {
                    if !known[dim.0 as usize] {
                        return Err(Defect::ResultSizeUnknown(*dim));
                    }
                }
            }
            for slot in &port.planes {
                if !matches!(storage(*slot)?, Storage::Output(_) | Storage::Alias(_)) {
                    return Err(Defect::OutputPlaneNotResult(*slot));
                }
            }
        }
        Ok(Checked(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::{AxisKind, Fiber};
    use crate::program::StateId;

    const STATE: Name = Name::State(StateId(0));

    fn need(element_bytes: u32, capacity: Vec<Sizing>) -> Need {
        Need {
            memory: Memory::Device(0),
            element_bytes,
            align: 8,
            capacity,
        }
    }

    fn leaf(scalar: Scalar, plane: u32, offset: u32) -> Leaf {
        Leaf {
            scalar,
            plane,
            offset,
        }
    }

    /// A rank-1 Array of Reals in one plane.
    fn field(domain: Sizing, slot: u32) -> Port {
        Port {
            name: STATE,
            kind: Kind::Array {
                element: Fiber::Real,
                axes: vec![AxisKind::Closed],
            },
            shape: vec![domain],
            leaves: vec![leaf(Scalar::F64, 0, 0)],
            planes: vec![Slot(slot)],
        }
    }

    fn cells(count: u64) -> Sizing {
        Sizing::Fixed(Size::Domain(vec![Axis {
            origin: 0,
            extent: count,
        }]))
    }

    fn stage(writes: &[u32], determines: &[u32]) -> Stage {
        Stage {
            reads: Vec::new(),
            writes: writes.iter().map(|slot| Slot(*slot)).collect(),
            determines: determines.iter().map(|dim| Dim(*dim)).collect(),
        }
    }

    #[test]
    fn a_stage_that_writes_its_input_is_refused() {
        let unit = Unit {
            inputs: vec![field(cells(4), 0)],
            storage: vec![Storage::Input(need(8, vec![cells(4)]))],
            stages: vec![stage(&[0], &[])],
            ..Unit::default()
        };
        let defect = Defect::WritesShared {
            stage: 0,
            slot: Slot(0),
        };
        assert_eq!(unit.check().err(), Some(defect));
    }

    #[test]
    fn storage_capacity_is_known_before_the_stage_that_writes_it() {
        let domain = || Sizing::Runtime(Dim(0));
        let unit = |capacity, stages| Unit {
            outputs: vec![field(domain(), 0)],
            storage: vec![Storage::Output(need(8, vec![capacity]))],
            dimensions: vec![Dimension::Domain { rank: 1 }],
            stages,
            ..Unit::default()
        };
        let late = Defect::SizeNotYetKnown {
            stage: 0,
            dim: Dim(0),
        };
        // Nobody states the domain.
        let never = unit(domain(), vec![stage(&[0], &[])]).check();
        assert_eq!(never.err(), Some(late));
        // A stage cannot settle the capacity of storage it is already writing.
        let same = unit(domain(), vec![stage(&[0], &[0])]).check();
        assert_eq!(same.err(), Some(late));
        // One stage measures, the next fills.
        let measured = unit(domain(), vec![stage(&[], &[0]), stage(&[0], &[])]);
        assert!(measured.check().is_ok());
        // One stage fills storage sized for a bound and reports the actual domain.
        assert!(unit(cells(100), vec![stage(&[0], &[0])]).check().is_ok());
    }

    #[test]
    fn a_fiber_array_may_be_one_plane_of_structs_or_one_plane_per_component() {
        // An Array of a fiber holding a Real and an Int.
        let fiber = Fiber::Product(vec![
            ("mass".to_string(), Fiber::Real),
            ("id".to_string(), Fiber::Int),
        ]);
        let unit = |leaves, storage: Vec<Storage>| {
            let planes: Vec<u32> = (0..storage.len() as u32).collect();
            Unit {
                outputs: vec![Port {
                    name: STATE,
                    kind: Kind::Array {
                        element: fiber.clone(),
                        axes: vec![AxisKind::Closed],
                    },
                    shape: vec![cells(4)],
                    leaves,
                    planes: planes.iter().map(|slot| Slot(*slot)).collect(),
                }],
                stages: vec![stage(&planes, &[])],
                storage,
                ..Unit::default()
            }
        };
        let out = |element_bytes| Storage::Output(need(element_bytes, vec![cells(4)]));

        // One array of whole fibers: both leaves in one 16-byte element.
        let together = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 0, 8)];
        assert!(unit(together, vec![out(16)]).check().is_ok());

        // Separate arrays: one plane per component.
        let apart = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 1, 0)];
        assert!(unit(apart, vec![out(8), out(4)]).check().is_ok());

        // The Int placed on top of the Real.
        let clash = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 0, 4)];
        let overlap = Defect::LeavesOverlap {
            first: 0,
            second: 1,
        };
        assert_eq!(unit(clash, vec![out(16)]).check().err(), Some(overlap));

        // A Real in a 4-byte element.
        let wide = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 1, 0)];
        let outside = Defect::LeafOutsideElement { leaf: 0 };
        assert_eq!(
            unit(wide, vec![out(4), out(4)]).check().err(),
            Some(outside)
        );

        // A Real stored as an integer, and a leaf missing altogether.
        let mismatch = Some(Defect::KindMismatch(STATE));
        let wrong = vec![leaf(Scalar::I64, 0, 0), leaf(Scalar::I32, 0, 8)];
        assert_eq!(unit(wrong, vec![out(16)]).check().err(), mismatch);
        let short = vec![leaf(Scalar::F64, 0, 0)];
        assert_eq!(unit(short, vec![out(16)]).check().err(), mismatch);
    }

    #[test]
    fn an_optional_result_reports_presence_in_the_shape_record() {
        // The State of a program that starts absent: an Optional Array.
        let unit = |kind, capacity| Unit {
            outputs: vec![Port {
                name: STATE,
                kind,
                shape: vec![Sizing::Runtime(Dim(0)), Sizing::Runtime(Dim(1))],
                leaves: vec![leaf(Scalar::F64, 0, 0)],
                planes: vec![Slot(0)],
            }],
            storage: vec![Storage::Output(need(8, vec![capacity]))],
            dimensions: vec![Dimension::Presence, Dimension::Domain { rank: 1 }],
            stages: vec![stage(&[0], &[0, 1])],
            ..Unit::default()
        };
        let array = || Kind::Array {
            element: Fiber::Real,
            axes: vec![AxisKind::Closed],
        };
        let optional = || Kind::Optional(Box::new(array()));
        assert!(unit(optional(), cells(100)).check().is_ok());

        // Presence is not a size, so it cannot state how much storage to provide.
        let as_capacity = unit(optional(), Sizing::Fixed(Size::Presence(true))).check();
        assert_eq!(as_capacity.err(), Some(Defect::CapacityNotASize(Slot(0))));

        // An Optional never directly holds an Optional.
        let nested = unit(Kind::Optional(Box::new(optional())), cells(100)).check();
        assert_eq!(nested.err(), Some(Defect::NestedOptional(STATE)));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_is_refused_as_unsupported() {
        let unit = Unit {
            outputs: vec![Port {
                name: STATE,
                kind: Kind::Array {
                    element: Fiber::Mat {
                        rows: 3,
                        columns: 3,
                    },
                    axes: vec![AxisKind::Closed],
                },
                shape: vec![cells(4)],
                leaves: Vec::new(),
                planes: vec![Slot(0)],
            }],
            storage: vec![Storage::Output(need(72, vec![cells(4)]))],
            stages: vec![stage(&[0], &[])],
            ..Unit::default()
        };
        assert_eq!(unit.check().err(), Some(Defect::KindUnsupported(STATE)));
    }
}
