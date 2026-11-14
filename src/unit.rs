//! The description of one piece of compiled code.

use crate::kind::{KindDefect, KindRef, Kinds, Primitive, ShapeField};
use crate::program::Name;

/// One axis of an Array domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub origin: i64,
    pub extent: u64,
}

/// What a shape field holds once it is known: a size, or presence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeValue {
    /// Whether an Optional value is present. Not a size; it counts nothing.
    Presence(bool),
    Length(u64),
    Domain(Vec<Axis>),
}

impl ShapeValue {
    /// How many elements this size describes, or nothing if it is presence
    /// or the count does not fit in 64 bits.
    pub fn count(&self) -> Option<u64> {
        match self {
            ShapeValue::Presence(_) => None,
            ShapeValue::Length(length) => Some(*length),
            ShapeValue::Domain(axes) if axes.iter().any(|axis| axis.extent == 0) => Some(0),
            ShapeValue::Domain(axes) => axes
                .iter()
                .try_fold(1u64, |count, axis| count.checked_mul(axis.extent)),
        }
    }

    pub fn is(&self, field: ShapeField) -> bool {
        match (self, field) {
            (ShapeValue::Presence(_), ShapeField::Presence) => true,
            (ShapeValue::Length(_), ShapeField::Length) => true,
            (ShapeValue::Domain(axes), ShapeField::Domain { rank }) => axes.len() == rank,
            _ => false,
        }
    }
}

/// Position of one shape field in a unit's shape record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeRef(pub u32);

/// A size or presence that is either fixed in the code or found in the shape
/// record.
///
/// The shape record is one table per run of a unit, with one entry for each of
/// the unit's shape fields. The runtime fills the entries that describe inputs.
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
pub enum ShapeEntry {
    Fixed(ShapeValue),
    Runtime(ShapeRef),
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
/// Gap: a scalar here says how a leaf is stored. It does not say at what
/// width arithmetic on it is done. Int arithmetic wraps at the Int's width
/// and Real arithmetic depends on precision, so that is not a layout choice
/// and nothing here grants the freedom to change it.
///
/// An Int is stored as `I32` or `I64` and a Real as `F32` or `F64`. A Bool
/// is stored as `I32` holding 0 for false or 1 for true; no other value is a
/// Bool. The check on a description cannot see the stored values, so keeping
/// to 0 and 1 is the duty of whoever writes one.
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
            Primitive::Bool => self == Scalar::I32,
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
    /// that first writes the slot.
    pub capacity: Vec<ShapeEntry>,
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
/// it: one entry for each of the kind's shape fields and one leaf for each of
/// its scalars, in the kind's order.
///
/// Decide: a value is bound to a port only if their layouts are identical.
/// Converting between layouts would be an explicit computation, never
/// something the runtime does silently while binding.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: Name,
    /// A reference into the unit's table of kinds.
    pub kind: KindRef,
    /// One entry per shape field of the kind, in the kind's order.
    pub shape: Vec<ShapeEntry>,
    /// One entry per scalar of the kind, in the kind's order.
    ///
    /// Gap: listing every scalar writes a repeated fiber out in full, so a
    /// vector of vectors grows with the product of its widths. A form that
    /// states a repeated layout once is needed before wide or nested vectors
    /// are carried.
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
/// No value is sized from its own contents. The size of a result may depend
/// on data, but only on data computed before the result is filled. One stage
/// determines the size, the runtime provides storage to fit, and a later
/// stage fills it. A stage therefore never determines the size or presence
/// of a value it is writing.
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
    pub determines: Vec<ShapeRef>,
}

/// Everything the runtime needs to know to run a piece of compiled code.
///
/// The job of a unit is to supply values at requested sites of indexed
/// values. The values it reads and the values it produces are held in
/// memory. That does not require a fresh copy; see `Storage::Alias`.
///
/// A value's domain is where it has values at all. It is distinct from the
/// sites a unit is asked to fill on one run, and from the sites that storage
/// currently covers. A size in a port is the value's domain.
///
/// Gap: how the sites asked for and the sites storage covers are stated,
/// when they are not the whole domain, is not written yet.
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
    /// Every kind the ports use, each stated once.
    pub kinds: Kinds,
    pub inputs: Vec<Port>,
    pub outputs: Vec<Port>,
    pub storage: Vec<Storage>,
    /// The layout of the shape record: every size not fixed at compile time.
    pub shape_fields: Vec<ShapeField>,
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
    UnknownShapeField(ShapeRef),
    /// A stage writes storage whose capacity, or a result whose size or
    /// presence, is settled by no input and no earlier stage. Storage is
    /// provided before a stage runs, and no value is sized from its own
    /// contents, so a stage cannot settle either for what it writes.
    ShapeNotYetKnown {
        stage: u32,
        field: ShapeRef,
    },
    /// A result's size is never determined.
    ResultShapeUnknown(ShapeRef),
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
    /// The unit's table of kinds is not well formed.
    Kind(KindDefect),
    /// A port refers to a kind that is not in the unit's table.
    UnknownKind(Name),
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
/// Gap: the check ties a result's sizes and presence to the whole port. It
/// should tie them to the part of the value a stage is writing. As it
/// stands, a result fixed as absent is refused for sizes that do not apply
/// to it, and a Struct with two Arrays cannot fill one, compute the other's
/// domain from it, then fill the other.
///
/// Gap: a defect names a leaf by its number but not its port, and several
/// distinct mismatches share one defect.
///
/// The field is private, so holding one is proof the description passed these
/// checks: the table of kinds is well formed and states each kind once;
/// every slot and shape field named exists; each port's kind has its storage
/// described, and its sizes,
/// presence and leaves match it; no capacity is stated as presence; no stage writes an input
/// or an alias; an alias refers to an input; the capacity of storage, and the
/// sizes and presence of a result, are known before the stage that writes it; every leaf fits inside one element of its plane and
/// overlaps no other. Passing these checks does not make a unit ready to
/// run. They do not show that every result is written by some stage, that
/// storage is read only after something wrote it, that a fixed domain fits
/// the capacity stated for it, or that an alignment is usable, and they
/// cannot show that the code honors the description.
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
        let exists = |entry: &ShapeEntry| match entry {
            ShapeEntry::Runtime(field) if field.0 as usize >= self.shape_fields.len() => {
                Err(Defect::UnknownShapeField(*field))
            }
            _ => Ok(()),
        };
        for (index, entry) in self.storage.iter().enumerate() {
            match entry {
                Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need) => {
                    for entry in &need.capacity {
                        exists(entry)?;
                        let presence = match entry {
                            ShapeEntry::Fixed(value) => matches!(value, ShapeValue::Presence(_)),
                            ShapeEntry::Runtime(field) => {
                                self.shape_fields[field.0 as usize] == ShapeField::Presence
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

        // Each kind is stated once and well formed, and a port's sizes and
        // leaves are the ones its kind calls for.
        self.kinds.check().map_err(Defect::Kind)?;
        let counts = self.kinds.leaf_counts();
        let shape_counts = self.kinds.shape_field_counts();
        for port in self.inputs.iter().chain(&self.outputs) {
            let mismatch = Defect::KindMismatch(port.name);
            let Some(count) = counts.get(port.kind.0 as usize) else {
                return Err(Defect::UnknownKind(port.name));
            };
            let Some(count) = *count else {
                return Err(Defect::KindUnsupported(port.name));
            };
            let facts = shape_counts[port.kind.0 as usize];
            if facts != Some(port.shape.len() as u64) || count != port.leaves.len() as u64 {
                return Err(mismatch);
            }
            let shape_fields = self.kinds.shape_fields(port.kind);
            let Some(primitives) = self.kinds.leaves(port.kind) else {
                return Err(Defect::KindUnsupported(port.name));
            };
            for (field, entry) in shape_fields.iter().zip(&port.shape) {
                let fits = match entry {
                    ShapeEntry::Fixed(value) => value.is(*field),
                    ShapeEntry::Runtime(at) => self.shape_fields.get(at.0 as usize) == Some(field),
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

        // Every leaf sits inside one element of its storage, overlapping no
        // other. Leaves are compared by the storage they sit in, not by the
        // port's own numbering of its planes, so two planes naming one slot
        // or one input are seen as the same storage.
        for port in self.inputs.iter().chain(&self.outputs) {
            let mut placed: Vec<(Slot, u32, u32)> = Vec::new();
            for (index, leaf) in port.leaves.iter().enumerate() {
                let index = index as u32;
                let slot = port
                    .planes
                    .get(leaf.plane as usize)
                    .ok_or(Defect::UnknownPlane { leaf: index })?;
                let (backing, element) = match storage(*slot)? {
                    Storage::Alias(of) => (*of, storage(*of)?),
                    other => (*slot, other),
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
                let clash = placed.iter().position(|(other, start, stop)| {
                    *other == backing && leaf.offset < *stop && *start < end
                });
                if let Some(first) = clash {
                    return Err(Defect::LeavesOverlap {
                        first: first as u32,
                        second: index,
                    });
                }
                placed.push((backing, leaf.offset, end));
            }
        }

        // A size is known once an input supplies it or a stage determines it.
        let mut known = vec![false; self.shape_fields.len()];
        for port in &self.inputs {
            for entry in &port.shape {
                exists(entry)?;
                if let ShapeEntry::Runtime(field) = entry {
                    known[field.0 as usize] = true;
                }
            }
            for slot in &port.planes {
                if !matches!(storage(*slot)?, Storage::Input(_)) {
                    return Err(Defect::InputPlaneNotInput(*slot));
                }
            }
        }
        let mut provided = vec![false; self.storage.len()];
        let results_in = |slot: Slot| {
            self.outputs
                .iter()
                .filter(move |port| port.planes.contains(&slot))
                .flat_map(|port| port.shape.iter())
        };
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
                    for entry in need.capacity.iter().chain(results_in(*slot)) {
                        if let ShapeEntry::Runtime(field) = entry {
                            if !known[field.0 as usize] {
                                return Err(Defect::ShapeNotYetKnown {
                                    stage: index as u32,
                                    field: *field,
                                });
                            }
                        }
                    }
                }
            }
            for determined in &stage.determines {
                *known
                    .get_mut(determined.0 as usize)
                    .ok_or(Defect::UnknownShapeField(*determined))? = true;
            }
        }
        for port in &self.outputs {
            for entry in &port.shape {
                exists(entry)?;
                if let ShapeEntry::Runtime(field) = entry {
                    if !known[field.0 as usize] {
                        return Err(Defect::ResultShapeUnknown(*field));
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
    use crate::kind::{AxisKind, Definition};
    use crate::program::StateId;

    const STATE: Name = Name::State(StateId(0));

    fn need(element_bytes: u32, capacity: Vec<ShapeEntry>) -> Need {
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

    fn array(kinds: &mut Kinds, element: Definition) -> KindRef {
        let element = kinds.define(element);
        kinds.define(Definition::Array {
            element,
            axes: vec![AxisKind::Closed],
        })
    }

    /// A table holding a rank-1 Array of Reals, and a port of that kind in
    /// one plane.
    fn field(domain: ShapeEntry, slot: u32) -> (Kinds, Port) {
        let mut kinds = Kinds::default();
        let kind = array(&mut kinds, Definition::Real);
        let port = Port {
            name: STATE,
            kind,
            shape: vec![domain],
            leaves: vec![leaf(Scalar::F64, 0, 0)],
            planes: vec![Slot(slot)],
        };
        (kinds, port)
    }

    fn cells(count: u64) -> ShapeEntry {
        ShapeEntry::Fixed(ShapeValue::Domain(vec![Axis {
            origin: 0,
            extent: count,
        }]))
    }

    fn stage(writes: &[u32], determines: &[u32]) -> Stage {
        Stage {
            reads: Vec::new(),
            writes: writes.iter().map(|slot| Slot(*slot)).collect(),
            determines: determines.iter().map(|field| ShapeRef(*field)).collect(),
        }
    }

    #[test]
    fn a_stage_that_writes_its_input_is_refused() {
        let (kinds, port) = field(cells(4), 0);
        let unit = Unit {
            kinds,
            inputs: vec![port],
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
        let domain = || ShapeEntry::Runtime(ShapeRef(0));
        let unit = |capacity, stages| {
            let (kinds, port) = field(domain(), 0);
            Unit {
                kinds,
                outputs: vec![port],
                storage: vec![Storage::Output(need(8, vec![capacity]))],
                shape_fields: vec![ShapeField::Domain { rank: 1 }],
                stages,
                ..Unit::default()
            }
        };
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
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
        // No Array is sized from its own contents: a stage cannot determine
        // the domain of a result it is filling, whatever storage it was given.
        let own = unit(cells(100), vec![stage(&[0], &[0])]).check();
        assert_eq!(own.err(), Some(late));
    }

    /// An Array result of the given kind, laid out by the given leaves in
    /// the given planes.
    fn result(kinds: Kinds, kind: KindRef, leaves: Vec<Leaf>, storage: Vec<Storage>) -> Unit {
        let planes: Vec<u32> = (0..storage.len() as u32).collect();
        Unit {
            kinds,
            outputs: vec![Port {
                name: STATE,
                kind,
                shape: vec![cells(4)],
                leaves,
                planes: planes.iter().map(|slot| Slot(*slot)).collect(),
            }],
            stages: vec![stage(&planes, &[])],
            storage,
            ..Unit::default()
        }
    }

    /// The same, for an Array of one fiber that needs no other kind.
    fn result_of(element: Definition, leaves: Vec<Leaf>, storage: Vec<Storage>) -> Unit {
        let mut kinds = Kinds::default();
        let kind = array(&mut kinds, element);
        result(kinds, kind, leaves, storage)
    }

    fn out(element_bytes: u32) -> Storage {
        Storage::Output(need(element_bytes, vec![cells(4)]))
    }

    #[test]
    fn a_fiber_array_may_be_one_plane_of_structs_or_one_plane_per_component() {
        // An Array of a fiber holding a Real and an Int.
        let unit = |leaves, storage| {
            let mut kinds = Kinds::default();
            let real = kinds.define(Definition::Real);
            let int = kinds.define(Definition::Int);
            let members = vec![("mass".to_string(), real), ("id".to_string(), int)];
            let kind = array(&mut kinds, Definition::Product(members));
            result(kinds, kind, leaves, storage)
        };

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
    fn a_bool_is_stored_as_a_32_bit_integer() {
        let stored = |scalar| result_of(Definition::Bool, vec![leaf(scalar, 0, 0)], vec![out(8)]);
        assert!(stored(Scalar::I32).check().is_ok());
        let mismatch = Some(Defect::KindMismatch(STATE));
        assert_eq!(stored(Scalar::I64).check().err(), mismatch);
        assert_eq!(stored(Scalar::F32).check().err(), mismatch);
    }

    #[test]
    fn an_optional_result_reports_presence_in_the_shape_record() {
        // The State of a program that starts absent: an Optional Array.
        let unit = |nest: bool, capacity| {
            let mut kinds = Kinds::default();
            let array = array(&mut kinds, Definition::Real);
            let mut kind = kinds.define(Definition::Optional(array));
            if nest {
                kind = kinds.define(Definition::Optional(kind));
            }
            Unit {
                kinds,
                outputs: vec![Port {
                    name: STATE,
                    kind,
                    shape: vec![
                        ShapeEntry::Runtime(ShapeRef(0)),
                        ShapeEntry::Runtime(ShapeRef(1)),
                    ],
                    leaves: vec![leaf(Scalar::F64, 0, 0)],
                    planes: vec![Slot(0)],
                }],
                storage: vec![Storage::Output(need(8, vec![capacity]))],
                shape_fields: vec![ShapeField::Presence, ShapeField::Domain { rank: 1 }],
                stages: vec![stage(&[], &[0, 1]), stage(&[0], &[])],
                ..Unit::default()
            }
        };
        assert!(unit(false, ShapeEntry::Runtime(ShapeRef(1)))
            .check()
            .is_ok());

        // Presence is not a size, so it cannot state how much storage to provide.
        let as_capacity = unit(false, ShapeEntry::Fixed(ShapeValue::Presence(true))).check();
        assert_eq!(as_capacity.err(), Some(Defect::CapacityNotASize(Slot(0))));

        // An Optional never directly holds an Optional.
        let nested = unit(true, cells(100)).check();
        let defect = KindDefect::NestedOptional { at: KindRef(3) };
        assert_eq!(nested.err(), Some(Defect::Kind(defect)));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_is_refused_as_unsupported() {
        let matrix = Definition::Mat {
            rows: 3,
            columns: 3,
        };
        let unit = result_of(matrix, Vec::new(), vec![out(72)]);
        assert_eq!(unit.check().err(), Some(Defect::KindUnsupported(STATE)));
    }
}

#[cfg(test)]
mod review {
    use super::*;
    use crate::kind::{Definition, Kinds};
    use crate::program::StateId;

    const STATE: Name = Name::State(StateId(0));

    #[test]
    fn an_empty_domain_counts_zero_whatever_the_axis_order() {
        let domain = |extents: &[u64]| {
            ShapeValue::Domain(
                extents
                    .iter()
                    .map(|extent| Axis {
                        origin: 0,
                        extent: *extent,
                    })
                    .collect(),
            )
        };
        assert_eq!(domain(&[0, u64::MAX, 2]).count(), Some(0));
        assert_eq!(domain(&[u64::MAX, 2, 0]).count(), Some(0));
    }

    #[test]
    fn two_leaves_in_the_same_storage_overlap_whatever_the_plane_index() {
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let members = vec![("x".to_string(), real), ("y".to_string(), real)];
        let kind = kinds.define(Definition::Product(members));
        let leaf = |plane| Leaf {
            scalar: Scalar::F32,
            plane,
            offset: 0,
        };
        let need = Need {
            memory: Memory::Host,
            element_bytes: 4,
            align: 4,
            capacity: Vec::new(),
        };
        let unit = Unit {
            kinds,
            outputs: vec![Port {
                name: STATE,
                kind,
                shape: Vec::new(),
                leaves: vec![leaf(0), leaf(1)],
                planes: vec![Slot(0), Slot(0)],
            }],
            storage: vec![Storage::Output(need)],
            stages: vec![Stage {
                reads: Vec::new(),
                writes: vec![Slot(0)],
                determines: Vec::new(),
            }],
            ..Unit::default()
        };
        let overlap = Defect::LeavesOverlap {
            first: 0,
            second: 1,
        };
        assert_eq!(unit.check().err(), Some(overlap));
    }

    #[test]
    fn a_deeply_shared_kind_is_checked_without_walking_every_scalar() {
        let mut kinds = Kinds::default();
        let mut kind = kinds.define(Definition::Real);
        for _ in 0..40 {
            kind = kinds.define(Definition::Tuple(vec![kind, kind]));
        }
        let unit = Unit {
            kinds,
            outputs: vec![Port {
                name: STATE,
                kind,
                shape: Vec::new(),
                leaves: Vec::new(),
                planes: Vec::new(),
            }],
            ..Unit::default()
        };
        // 2^40 scalars and no leaves listed: refused at once, not after a walk.
        assert_eq!(unit.check().err(), Some(Defect::KindMismatch(STATE)));
    }

    #[test]
    fn every_result_sharing_storage_must_be_sized_before_it_is_written() {
        // Two results in one slot. The first has a fixed domain; the second's
        // is determined by the very stage that writes the slot.
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let kind = kinds.define(Definition::Array {
            element: real,
            axes: vec![crate::kind::AxisKind::Closed],
        });
        let cells = ShapeEntry::Fixed(ShapeValue::Domain(vec![Axis {
            origin: 0,
            extent: 4,
        }]));
        let port = |shape| Port {
            name: STATE,
            kind,
            shape: vec![shape],
            leaves: vec![Leaf {
                scalar: Scalar::F64,
                plane: 0,
                offset: 0,
            }],
            planes: vec![Slot(0)],
        };
        let unit = Unit {
            kinds,
            outputs: vec![port(cells.clone()), port(ShapeEntry::Runtime(ShapeRef(0)))],
            storage: vec![Storage::Output(Need {
                memory: Memory::Host,
                element_bytes: 8,
                align: 8,
                capacity: vec![cells],
            })],
            shape_fields: vec![crate::kind::ShapeField::Domain { rank: 1 }],
            stages: vec![Stage {
                reads: Vec::new(),
                writes: vec![Slot(0)],
                determines: vec![ShapeRef(0)],
            }],
            ..Unit::default()
        };
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        assert_eq!(unit.check().err(), Some(late));
    }
}
