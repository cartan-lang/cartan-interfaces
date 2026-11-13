//! The description of one piece of compiled code.

use crate::program::Name;

/// Logical type of a value: its compile-time shape.
///
/// Gap: this stands for the full kind tree (scalars, Arrays, Structs, Tuples,
/// Sums, Optional values, Lists, Maps). The tree and its equality rule are not
/// written yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema(pub u32);

/// One axis of an Array domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub origin: i64,
    pub extent: u64,
}

/// A kind of size that is only known at run time.
///
/// A value's kind fixes its structure at compile time: which planes it has and
/// how many bytes each element takes. What compile time cannot fix is how many
/// elements there are. Those counts are the dimensions.
///
/// The topology of an Array axis and the presence of an Optional value are
/// also known only at run time, but they are not dimensions. They are
/// metadata of a value. Neither counts elements, and an absent value is not an
/// empty one.
///
/// Gap: how a port and a value carry topology and presence is not written yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    /// The number of elements in a List.
    Length,
    /// The domain of an Array: an origin and an extent for each axis.
    Domain { rank: u8 },
}

/// What a dimension holds once it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Size {
    Length(u64),
    Domain(Vec<Axis>),
}

impl Size {
    /// How many elements this size describes, or nothing if the count does
    /// not fit in 64 bits.
    pub fn count(&self) -> Option<u64> {
        match self {
            Size::Length(length) => Some(*length),
            Size::Domain(axes) => axes
                .iter()
                .try_fold(1u64, |count, axis| count.checked_mul(axis.extent)),
        }
    }

    pub fn is(&self, dimension: Dimension) -> bool {
        match (self, dimension) {
            (Size::Length(_), Dimension::Length) => true,
            (Size::Domain(axes), Dimension::Domain { rank }) => axes.len() == rank as usize,
            _ => false,
        }
    }
}

/// Position of one dimension in a unit's shape record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dim(pub u32);

/// A size that is either fixed in the code or found in the shape record.
///
/// The shape record is one table per run of a unit, with one entry for each of
/// the unit's dimensions. The runtime fills the entries that describe inputs.
/// The unit's stages fill the rest, and that is how a unit states how much
/// storage it needs.
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

/// A machine scalar: the type of one leaf of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    I32,
    I64,
    F32,
    F64,
}

impl Scalar {
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
/// compile-time element size times run-time counts.
///
/// A unit never asks for a number of bytes. It states sizes in the shape
/// record, and the bytes follow from them.
#[derive(Clone, Debug)]
pub struct Need {
    pub memory: Memory,
    pub element_bytes: u32,
    pub align: u32,
    /// The element count is the product of these. Empty means one element.
    pub count: Vec<Sizing>,
}

/// What each slot is for. This decides who may write it.
#[derive(Clone, Debug)]
pub enum Storage {
    /// A plane of an input value: the memory, element size and element count
    /// the unit expects to find. Never written.
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

/// One value entering or leaving a unit: its compile-time kind, its sizes and
/// its physical layout.
///
/// Decide: a value is bound to a port only if their layouts are identical.
/// Converting between layouts would be an explicit computation, never
/// something the runtime does silently while binding.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: Name,
    pub schema: Schema,
    /// One entry per List or Array in the value, in the order the kind lists them.
    pub shape: Vec<Sizing>,
    /// One entry per scalar leaf of the value, in the order the kind lists them.
    pub leaves: Vec<Leaf>,
    /// The slots that hold the value. Each is one physical plane.
    pub planes: Vec<Slot>,
}

/// One entry point in the code. Stages run in order.
///
/// A unit asks for storage by stating sizes: it writes them into the shape
/// record. The runtime provides the storage a stage writes as soon as its
/// sizes are known. If an input or an earlier stage settled them, the storage
/// is there when the stage starts. If the stage itself settles them, because
/// the size depends on data it computes, the stage asks while running and
/// waits until the runtime has provided the storage.
///
/// A kernel on a GPU cannot wait part-way through. On Metal and CUDA a result
/// sized by data needs one stage to measure and a later stage to fill.
///
/// A stage has no failure to report. Cartan is total, so compiled code always
/// runs to completion and every outcome it computes is an ordinary value. A
/// run can still end early, but only for reasons the runtime discovers: it
/// cannot provide the storage a stage asked for, or the run is cancelled. A
/// stage whose request is refused is not resumed.
///
/// Gap: the calling convention of a stage on each target is not written yet:
/// how the entry is named, how slots and the shape record are passed, how a
/// running stage asks for storage and waits, how a stage waiting on a refused
/// request is abandoned, and the extent a parallel stage is dispatched over.
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
    /// A stage writes storage whose size is settled by no input, no earlier
    /// stage and not by the stage itself.
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
/// checks: every slot and dimension it names exists; no stage writes an input
/// or an alias; an alias refers to an input; storage is written only once its
/// size has been stated; every leaf fits inside one element of its plane and
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
                    need.count.iter().try_for_each(dim)?
                }
                Storage::Alias(of) => {
                    if !matches!(storage(*of)?, Storage::Input(_)) {
                        return Err(Defect::AliasOfNonInput(Slot(index as u32)));
                    }
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
            for determined in &stage.determines {
                *known
                    .get_mut(determined.0 as usize)
                    .ok_or(Defect::UnknownDimension(*determined))? = true;
            }
            for slot in &stage.writes {
                let (Storage::Output(need) | Storage::Scratch(need)) = storage(*slot)? else {
                    return Err(Defect::WritesShared {
                        stage: index as u32,
                        slot: *slot,
                    });
                };
                if !std::mem::replace(&mut provided[slot.0 as usize], true) {
                    for sizing in &need.count {
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
    use crate::program::StateId;

    fn need(element_bytes: u32, count: Vec<Sizing>) -> Need {
        Need {
            memory: Memory::Device(0),
            element_bytes,
            align: 8,
            count,
        }
    }

    fn port(shape: Vec<Sizing>, leaves: Vec<Leaf>, planes: &[u32]) -> Port {
        Port {
            name: Name::State(StateId(0)),
            schema: Schema(0),
            shape,
            leaves,
            planes: planes.iter().map(|slot| Slot(*slot)).collect(),
        }
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
            inputs: vec![port(Vec::new(), Vec::new(), &[0])],
            storage: vec![Storage::Input(need(8, Vec::new()))],
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
    fn storage_needs_a_size_from_an_input_an_earlier_stage_or_the_stage_itself() {
        let length = || vec![Sizing::Runtime(Dim(0))];
        let unit = |stages| Unit {
            outputs: vec![port(length(), Vec::new(), &[0])],
            storage: vec![Storage::Output(need(8, length()))],
            dimensions: vec![Dimension::Length],
            stages,
            ..Unit::default()
        };
        let defect = Defect::SizeNotYetKnown {
            stage: 0,
            dim: Dim(0),
        };
        // Nobody states the length.
        assert_eq!(unit(vec![stage(&[0], &[])]).check().err(), Some(defect));
        // One stage measures, the next fills.
        assert!(unit(vec![stage(&[], &[0]), stage(&[0], &[])])
            .check()
            .is_ok());
        // One stage sizes its own result from data it computes.
        assert!(unit(vec![stage(&[0], &[0])]).check().is_ok());
    }

    #[test]
    fn a_fiber_array_may_be_one_plane_of_structs_or_one_plane_per_component() {
        // A fiber of an f64 and an i32, as an Array result.
        let leaf = |scalar, plane, offset| Leaf {
            scalar,
            plane,
            offset,
        };
        let unit = |leaves, storage: Vec<Storage>| {
            let planes: Vec<u32> = (0..storage.len() as u32).collect();
            Unit {
                outputs: vec![port(Vec::new(), leaves, &planes)],
                stages: vec![stage(&planes, &[])],
                storage,
                ..Unit::default()
            }
        };
        let out = |element_bytes| Storage::Output(need(element_bytes, Vec::new()));

        // One array of whole fibers: both leaves in one 16-byte element.
        let together = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 0, 8)];
        assert!(unit(together, vec![out(16)]).check().is_ok());

        // Separate arrays: one plane per component.
        let apart = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 1, 0)];
        assert!(unit(apart, vec![out(8), out(4)]).check().is_ok());

        // The i32 placed on top of the f64.
        let clash = vec![leaf(Scalar::F64, 0, 0), leaf(Scalar::I32, 0, 4)];
        let overlap = Defect::LeavesOverlap {
            first: 0,
            second: 1,
        };
        assert_eq!(unit(clash, vec![out(16)]).check().err(), Some(overlap));

        // An f64 in a 4-byte element.
        let wide = vec![leaf(Scalar::F64, 0, 0)];
        let outside = Defect::LeafOutsideElement { leaf: 0 };
        assert_eq!(unit(wide, vec![out(4)]).check().err(), Some(outside));
    }
}
