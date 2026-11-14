//! The description of one piece of compiled code.

use crate::kind::{Definition, KindDefect, KindRef, Kinds, ShapeField};
use crate::layout::{Layout, LayoutDefect, LayoutRef, Layouts, Measures};
use crate::program::Root;

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
/// runs. It can know the presence of a result only once the stage that
/// determines it has completed, so storage it provided beforehand may go
/// unused. The sizes of an absent value are left unfilled.
///
/// In an input port, `Fixed` is a specialization the runtime verifies against
/// the actual value before running. A change of extent needs no recompilation
/// unless the code chose to specialize on it.
///
/// Gap: where the record is kept is not written. A size or presence belongs
/// to the value it describes, and a value kept after a run needs its sizes
/// kept with it, so a table that lasts one run cannot be the only place they
/// live. Whether the record, or part of it, may sit in device or shared
/// memory, so that later device code reads a size without the host, is not
/// designed.
///
/// Gap: on a device, the work of a stage may be submitted well before it has
/// completed. This contract does not yet separate the two. It says in what
/// order sizes and presence are determined, not when the host, or later code
/// on the device, can read them.
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

/// One entry in a unit's storage table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(pub u32);

/// What storage a slot needs: which memory, and how much of it as a
/// compile-time element size times a run-time capacity.
///
/// A unit never asks for a number of bytes, and it never asks at all: this
/// description is the request. The runtime reads it, with the sizes in the
/// shape record, and provides the storage before the stage that writes it.
///
/// The storage a slot is given may be a region of a larger allocation. The
/// runtime passes it as a base and an offset from that base, and a whole
/// allocation is the case where the offset is zero. The region starts where
/// `align` allows and holds the stated capacity. Code may not assume that a
/// slot begins an allocation or that two slots lie in different ones, and it
/// reads and writes nothing outside the regions it was given. The regions of
/// different slots do not overlap, except where one slot is an alias.
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
    /// A plane of a computed result that is an input plane unchanged. Shares
    /// its storage. A result that is a whole input value is a `Selection`.
    Alias(Slot),
    /// Private to one run of the unit. It has no identity outside it.
    Scratch(Need),
}

/// One value entering or leaving a unit: its kind, its layout, its sizes and
/// the storage that holds it. The kind says what the value is and the layout
/// says how it is kept. The layout must lay out that kind, and the port gives
/// one size or presence for each run-time fact of the kind and one slot for
/// each plane of the layout.
///
/// Decide: a value is bound to a port only if their layouts are identical.
/// Converting between layouts would be an explicit computation, never
/// something the runtime does silently while binding.
#[derive(Clone, Debug)]
pub struct Port {
    /// What this port carries. An input is always the value of a node; a
    /// result answers one root the unit was asked for.
    pub root: Root,
    /// A reference into the unit's table of kinds.
    pub kind: KindRef,
    /// A reference into the unit's table of layouts.
    pub layout: LayoutRef,
    /// One entry per shape field of the kind, in the kind's order.
    pub shape: Vec<ShapeEntry>,
    /// The slots that hold the value, one for each plane of the layout. No
    /// two planes of one value share storage.
    pub planes: Vec<Slot>,
}

/// A result that is one of the unit's inputs, unchanged.
///
/// The result is the value itself: its sizes, its layout and its storage.
/// Nothing is computed and nothing is copied.
///
/// With no `otherwise`, the result is the input `first`. This is how a unit
/// answers with a State as it stands.
///
/// With an `otherwise`, `first` is an Optional input. When it is present the
/// result is what it holds, and when it is absent the result is the input
/// `otherwise`. The runtime knows which before the unit runs, because the
/// presence of an input is known then. The two must be kept the same way,
/// so the result has one layout whichever is chosen. They need not have the
/// same sizes: the result has the sizes of the one chosen.
///
/// An input that the unit uses only as the `otherwise` of selections is
/// needed only when one of them would choose it. While the Optional input is
/// present the runtime need not hold it at all.
///
/// The runtime makes the choice, so it must be able to read the presence of
/// `first` before the unit runs, and it waits if it cannot yet.
///
/// Gap: choosing on a device, or demanding the fallback conditionally there,
/// is not designed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// What this result answers.
    pub root: Root,
    /// A reference into the unit's table of kinds.
    pub kind: KindRef,
    /// Which input is chosen: always, or when it is present.
    pub first: u32,
    /// Which input is chosen when `first` is absent.
    pub otherwise: Option<u32>,
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
/// A stage need not read storage to determine a size. It may work one out
/// from sizes and presence already in the shape record, such as a domain two
/// cells smaller than an input's.
///
/// Gap: a stage lists the storage it reads but not the sizes and presence it
/// reads, nor which of them set the extent it is dispatched over. Without
/// that, a description cannot show what a size depends on, and the runtime
/// cannot tell what must be available, and where, before the stage starts.
///
/// Gap: nothing says where a stage runs. The code is emitted for one target,
/// so a stage whose inputs are all available on the host cannot be placed
/// there.
///
/// Each slot is passed to a stage as a base and an offset.
///
/// Gap: the rest of the calling convention of a stage on each target is not
/// written yet: what a base is, how the entry is named, how the shape record
/// is passed, and the extent a parallel stage is dispatched over.
#[derive(Clone, Debug)]
pub struct Stage {
    pub reads: Vec<Slot>,
    pub writes: Vec<Slot>,
    /// Dimensions this stage writes into the shape record.
    pub determines: Vec<ShapeRef>,
}

/// Everything the runtime needs to know to run a piece of compiled code.
///
/// A unit's results are a function of its inputs and of nothing else.
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
/// is decided by the runtime, not by anything in the unit. Producing the value
/// of a State is not the same as writing it.
#[derive(Clone, Debug, Default)]
pub struct Unit {
    /// Every kind the ports use, each stated once.
    pub kinds: Kinds,
    /// Every layout the ports use, each stated once.
    pub layouts: Layouts,
    pub inputs: Vec<Port>,
    /// Results the unit computes.
    pub outputs: Vec<Port>,
    /// Results that are inputs, unchanged.
    pub selected: Vec<Selection>,
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

/// One port of a unit, by its place among the inputs or the results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortRef {
    Input(u32),
    Output(u32),
    Selected(u32),
}

/// Why a unit description is not well formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    UnknownSlot(Slot),
    UnknownShapeField(ShapeRef),
    /// A stage writes storage whose capacity, or part of a result whose size
    /// or presence, is settled by no input and no earlier stage. Storage is
    /// provided before a stage runs, and no value is sized from its own
    /// contents, so a stage cannot settle either for what it writes.
    ShapeNotYetKnown {
        stage: u32,
        field: ShapeRef,
    },
    /// A size or presence of a result is never determined.
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
    /// The unit's table of kinds is not well formed.
    Kind(KindDefect),
    /// The unit's table of layouts is not well formed.
    Layout(LayoutDefect),
    /// A port refers to a kind that is not in the unit's table.
    UnknownKind(PortRef),
    /// A port refers to a layout that is not in the unit's table.
    UnknownLayout(PortRef),
    /// A port's kind includes a fiber whose storage this contract does not
    /// describe yet. The kind is valid; a unit cannot carry it across.
    KindUnsupported(PortRef),
    /// A port does not match its kind: its layout does not lay out that
    /// kind, it gives the wrong number of sizes or of slots, or a size is of
    /// the wrong sort.
    KindMismatch(PortRef),
    /// An input port is not the value of a node.
    InputNotAValue(PortRef),
    /// A selection names an input the unit does not have.
    UnknownInput(PortRef),
    /// A storage capacity is stated as presence, which is not a size.
    CapacityNotASize(Slot),
    /// Two planes of one value are given the same storage.
    PlaneRepeated {
        port: PortRef,
        plane: u32,
    },
    /// A plane's layout spans more bytes than one element of its storage
    /// holds.
    OutsideElement {
        port: PortRef,
        plane: u32,
    },
    /// Parts of a value with different sizes or presence are placed in one
    /// plane. A plane holds one run of elements, so everything in it shares
    /// one domain and one presence.
    PlaneSharedAcrossParts {
        port: PortRef,
        plane: u32,
    },
}

/// A unit description that passed [`Unit::check`].
///
/// The field is private, so holding one is proof the description passed these
/// checks. The tables of kinds and of layouts are well formed and state each
/// entry once. Every slot and shape field named exists. Each port's kind has
/// its storage described, its layout lays out that kind, and it gives one
/// size or presence for each run-time fact and one slot for each plane. A
/// selection chooses among inputs of the right kinds that are kept one way. No
/// two planes of a value share storage, and each plane's layout fits inside
/// one element of its storage. No capacity is stated as presence. No stage
/// writes an input or an alias, and an alias refers to an input. The
/// capacity of storage is known before the stage that writes it, and so are
/// the sizes and presence of the part of a result kept in it. Every size and
/// presence of a result is determined somewhere, except those of a part
/// fixed as absent, which do not apply.
///
/// Passing these checks does not make a unit ready to run. They do not show
/// that every result is written by some stage, that storage is read only
/// after something wrote it, that a fixed domain fits the capacity stated for
/// it, or that an alignment is usable, and they cannot show that the code
/// honors the description.
///
/// The checks on sizes establish the order in which they are determined.
/// They do not establish that the runtime can read a size at that point, or
/// that a device has finished writing it.
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

/// Which sizes and presence govern each plane of one port.
struct Governed<'a> {
    layouts: &'a Layouts,
    measures: &'a Measures,
    shape: &'a [ShapeEntry],
    /// For each plane, the facts that govern it, once something is placed
    /// there.
    over: Vec<Option<Vec<usize>>>,
    /// Planes that hold part of a value fixed as absent.
    absent_planes: Vec<bool>,
    /// Facts that belong to a part of a value fixed as absent.
    absent_facts: Vec<bool>,
}

impl Governed<'_> {
    /// Walk one layout placed at a plane, with its facts starting at `fact`.
    /// Only parts that have facts of their own are entered; everything else
    /// is one run of planes. The error is a plane two differently governed
    /// parts were placed in.
    fn walk(
        &mut self,
        layout: LayoutRef,
        plane: u32,
        fact: usize,
        enclosing: &mut Vec<usize>,
        absent: bool,
    ) -> Result<(), u32> {
        let place = layout.0 as usize;
        let planes = self.measures.planes[place];
        let facts = self.measures.facts[place] as usize;
        if absent {
            self.absent_facts[fact..fact + facts].fill(true);
        }
        match self.layouts.get(layout) {
            Some(Layout::Optional(inner)) => {
                let fixed_absent = ShapeEntry::Fixed(ShapeValue::Presence(false));
                let absent = absent || self.shape[fact] == fixed_absent;
                enclosing.push(fact);
                self.walk(*inner, plane, fact + 1, enclosing, absent)?;
                enclosing.pop();
            }
            Some(Layout::Array(_)) => {
                enclosing.push(fact);
                self.place(plane, planes, enclosing, absent)?;
                enclosing.pop();
            }
            Some(Layout::Group(parts)) if facts > 0 => {
                let mut next = fact;
                for part in parts {
                    self.walk(part.layout, plane + part.plane, next, enclosing, absent)?;
                    next += self.measures.facts[part.layout.0 as usize] as usize;
                }
            }
            Some(Layout::Repeat {
                part,
                count,
                plane_step,
                ..
            }) if facts > 0 => {
                let each = self.measures.facts[part.0 as usize] as usize;
                for copy in 0..*count {
                    let plane = plane + copy * plane_step;
                    self.walk(*part, plane, fact + copy as usize * each, enclosing, absent)?;
                }
            }
            _ => self.place(plane, planes, enclosing, absent)?,
        }
        Ok(())
    }

    fn place(
        &mut self,
        plane: u32,
        planes: u32,
        enclosing: &[usize],
        absent: bool,
    ) -> Result<(), u32> {
        for plane in plane..plane + planes {
            let at = plane as usize;
            match &self.over[at] {
                None => self.over[at] = Some(enclosing.to_vec()),
                Some(already) if already == enclosing => {}
                Some(_) => return Err(plane),
            }
            self.absent_planes[at] = absent;
        }
        Ok(())
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

        // Each kind and each layout is stated once and well formed.
        self.kinds.check().map_err(Defect::Kind)?;
        let scalars = self.kinds.scalar_counts();
        let facts = self.kinds.shape_field_counts();
        let most_planes = u32::try_from(self.storage.len()).unwrap_or(u32::MAX);
        let measures = self.layouts.measure(most_planes).map_err(Defect::Layout)?;

        // A size is known once an input supplies it or a stage determines it.
        let mut known = vec![false; self.shape_fields.len()];
        // What must be known before each slot is written, and what must be
        // known by the end.
        let mut before: Vec<Vec<ShapeRef>> = vec![Vec::new(); self.storage.len()];
        let mut owed: Vec<ShapeRef> = Vec::new();

        let inputs = self.inputs.iter().enumerate();
        let outputs = self.outputs.iter().enumerate();
        let ports = inputs
            .map(|(index, port)| (PortRef::Input(index as u32), port))
            .chain(outputs.map(|(index, port)| (PortRef::Output(index as u32), port)));
        for (at, port) in ports {
            let input = matches!(at, PortRef::Input(_));
            if input && !matches!(port.root, Root::Value(_)) {
                return Err(Defect::InputNotAValue(at));
            }

            // The port's layout lays out its kind, and it gives one entry
            // for each fact and one slot for each plane.
            let kind = port.kind.0 as usize;
            let layout = port.layout.0 as usize;
            match scalars.get(kind) {
                None => return Err(Defect::UnknownKind(at)),
                Some(None) => return Err(Defect::KindUnsupported(at)),
                Some(Some(_)) => {}
            }
            if layout >= self.layouts.definitions.len() {
                return Err(Defect::UnknownLayout(at));
            }
            let entries = port.shape.len() as u64;
            let mismatch = Defect::KindMismatch(at);
            if facts[kind] != Some(entries)
                || measures.facts[layout] != entries
                || measures.planes[layout] as usize != port.planes.len()
                || !self.layouts.fits(port.layout, &self.kinds, port.kind)
            {
                return Err(mismatch);
            }
            for (field, entry) in self.kinds.shape_fields(port.kind).iter().zip(&port.shape) {
                exists(entry)?;
                let fits = match entry {
                    ShapeEntry::Fixed(value) => value.is(*field),
                    ShapeEntry::Runtime(at) => self.shape_fields[at.0 as usize] == *field,
                };
                if !fits {
                    return Err(mismatch);
                }
            }

            // Each plane has storage of the right sort, to itself, with
            // elements large enough for what the layout keeps there.
            let mut used: Vec<Slot> = Vec::new();
            for (plane, slot) in port.planes.iter().enumerate() {
                let stated = storage(*slot)?;
                let right_sort = match stated {
                    Storage::Input(_) => input,
                    Storage::Output(_) | Storage::Alias(_) => !input,
                    Storage::Scratch(_) => false,
                };
                if !right_sort && input {
                    return Err(Defect::InputPlaneNotInput(*slot));
                } else if !right_sort {
                    return Err(Defect::OutputPlaneNotResult(*slot));
                }
                let (backing, held) = match stated {
                    Storage::Alias(of) => (*of, storage(*of)?),
                    other => (*slot, other),
                };
                let (Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need)) = held
                else {
                    return Err(Defect::AliasOfNonInput(*slot));
                };
                let plane = plane as u32;
                if used.contains(&backing) {
                    return Err(Defect::PlaneRepeated { port: at, plane });
                }
                used.push(backing);
                if measures.spans[layout][plane as usize].1 > need.element_bytes {
                    return Err(Defect::OutsideElement { port: at, plane });
                }
            }

            // Which sizes and presence govern each plane.
            let mut governed = Governed {
                layouts: &self.layouts,
                measures: &measures,
                shape: &port.shape,
                over: vec![None; port.planes.len()],
                absent_planes: vec![false; port.planes.len()],
                absent_facts: vec![false; port.shape.len()],
            };
            governed
                .walk(port.layout, 0, 0, &mut Vec::new(), false)
                .map_err(|plane| Defect::PlaneSharedAcrossParts { port: at, plane })?;
            let runtime = |fact: usize| match &port.shape[fact] {
                ShapeEntry::Runtime(field) => Some(*field),
                ShapeEntry::Fixed(_) => None,
            };
            if input {
                for field in (0..port.shape.len()).filter_map(runtime) {
                    known[field.0 as usize] = true;
                }
                continue;
            }
            for (plane, slot) in port.planes.iter().enumerate() {
                if !governed.absent_planes[plane] {
                    let over = governed.over[plane].iter().flatten();
                    before[slot.0 as usize].extend(over.filter_map(|fact| runtime(*fact)));
                }
            }
            let applies = (0..port.shape.len()).filter(|fact| !governed.absent_facts[*fact]);
            owed.extend(applies.filter_map(runtime));
        }

        // A selection chooses among inputs of the right kinds, kept one way.
        for (index, selection) in self.selected.iter().enumerate() {
            let at = PortRef::Selected(index as u32);
            let input = |index: u32| {
                self.inputs
                    .get(index as usize)
                    .ok_or(Defect::UnknownInput(at))
            };
            let first = input(selection.first)?;
            let fits = match selection.otherwise {
                None => first.kind == selection.kind,
                Some(otherwise) => {
                    let otherwise = input(otherwise)?;
                    let holds = match self.kinds.get(first.kind) {
                        Some(Definition::Optional(holds)) => Some(*holds),
                        _ => None,
                    };
                    let kept = match self.layouts.get(first.layout) {
                        Some(Layout::Optional(kept)) => Some(*kept),
                        _ => None,
                    };
                    // The other input is either what `first` may hold, or
                    // itself an Optional of it; the result is of its kind.
                    let whole = holds == Some(otherwise.kind) && kept == Some(otherwise.layout);
                    let optional = holds.is_some()
                        && first.kind == otherwise.kind
                        && first.layout == otherwise.layout;
                    otherwise.kind == selection.kind && (whole || optional)
                }
            };
            if !fits {
                return Err(Defect::KindMismatch(at));
            }
        }

        // A stage writes only storage it owns, whose capacity is known, and
        // only parts of results whose sizes and presence are known.
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
                    let capacity = need.capacity.iter().filter_map(|entry| match entry {
                        ShapeEntry::Runtime(field) => Some(*field),
                        ShapeEntry::Fixed(_) => None,
                    });
                    for field in capacity.chain(before[slot.0 as usize].iter().copied()) {
                        if !known[field.0 as usize] {
                            return Err(Defect::ShapeNotYetKnown {
                                stage: index as u32,
                                field,
                            });
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
        match owed.into_iter().find(|field| !known[field.0 as usize]) {
            Some(field) => Err(Defect::ResultShapeUnknown(field)),
            None => Ok(Checked(self)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::AxisKind;
    use crate::layout::{Part, Scalar};
    use crate::program::{Node, StateId};

    const STATE: Root = Root::Value(Node::State(StateId(0)));
    const RESULT: PortRef = PortRef::Output(0);

    fn need(element_bytes: u32, capacity: Vec<ShapeEntry>) -> Need {
        Need {
            memory: Memory::Device(0),
            element_bytes,
            align: 8,
            capacity,
        }
    }

    fn cells(count: u64) -> ShapeEntry {
        ShapeEntry::Fixed(ShapeValue::Domain(vec![Axis {
            origin: 0,
            extent: count,
        }]))
    }

    fn at(field: u32) -> ShapeEntry {
        ShapeEntry::Runtime(ShapeRef(field))
    }

    fn out(element_bytes: u32) -> Storage {
        Storage::Output(need(element_bytes, vec![cells(4)]))
    }

    fn stage(writes: &[u32], determines: &[u32]) -> Stage {
        Stage {
            reads: Vec::new(),
            writes: writes.iter().map(|slot| Slot(*slot)).collect(),
            determines: determines.iter().map(|field| ShapeRef(*field)).collect(),
        }
    }

    fn part(layout: LayoutRef, plane: u32, offset: u32) -> Part {
        Part {
            layout,
            plane,
            offset,
        }
    }

    fn slots(count: usize) -> Vec<Slot> {
        (0..count as u32).map(Slot).collect()
    }

    /// A unit under construction: its tables, and helpers that add to them.
    #[derive(Default)]
    struct Tables {
        kinds: Kinds,
        layouts: Layouts,
    }

    impl Tables {
        /// A rank-1 Array of one scalar fiber, stored as the given scalar.
        fn array(&mut self, element: Definition, stored: Scalar) -> (KindRef, LayoutRef) {
            let element = self.kinds.define(element);
            let kind = self.kinds.define(Definition::Array {
                element,
                axes: vec![AxisKind::Closed],
            });
            let scalar = self.layouts.define(Layout::Scalar(stored));
            (kind, self.layouts.define(Layout::Array(scalar)))
        }

        fn reals(&mut self) -> (KindRef, LayoutRef) {
            self.array(Definition::Real, Scalar::F64)
        }

        fn port(
            &self,
            kind: KindRef,
            layout: LayoutRef,
            shape: Vec<ShapeEntry>,
            planes: &[u32],
        ) -> Port {
            Port {
                root: STATE,
                kind,
                layout,
                shape,
                planes: planes.iter().map(|slot| Slot(*slot)).collect(),
            }
        }

        fn unit(self) -> Unit {
            Unit {
                kinds: self.kinds,
                layouts: self.layouts,
                ..Unit::default()
            }
        }
    }

    /// A unit with one Array of Reals as its only result, in slot 0.
    fn one_result(shape: ShapeEntry, capacity: ShapeEntry, stages: Vec<Stage>) -> Unit {
        let mut tables = Tables::default();
        let (kind, layout) = tables.reals();
        let port = tables.port(kind, layout, vec![shape], &[0]);
        Unit {
            outputs: vec![port],
            storage: vec![Storage::Output(need(8, vec![capacity]))],
            shape_fields: vec![ShapeField::Domain { rank: 1 }],
            stages,
            ..tables.unit()
        }
    }

    #[test]
    fn a_stage_that_writes_its_input_is_refused() {
        let mut tables = Tables::default();
        let (kind, layout) = tables.reals();
        let port = tables.port(kind, layout, vec![cells(4)], &[0]);
        let unit = Unit {
            inputs: vec![port],
            storage: vec![Storage::Input(need(8, vec![cells(4)]))],
            stages: vec![stage(&[0], &[])],
            ..tables.unit()
        };
        let defect = Defect::WritesShared {
            stage: 0,
            slot: Slot(0),
        };
        assert_eq!(unit.check().err(), Some(defect));
    }

    #[test]
    fn an_input_is_the_value_of_a_node() {
        // A unit may be asked for a State's initial value, but cannot take one in.
        let mut tables = Tables::default();
        let (kind, layout) = tables.reals();
        let mut port = tables.port(kind, layout, vec![cells(4)], &[0]);
        port.root = Root::Initial(StateId(0));
        let unit = Unit {
            inputs: vec![port],
            storage: vec![Storage::Input(need(8, vec![cells(4)]))],
            ..tables.unit()
        };
        let defect = Defect::InputNotAValue(PortRef::Input(0));
        assert_eq!(unit.check().err(), Some(defect));
    }

    #[test]
    fn capacity_and_sizes_are_known_before_the_stage_that_writes() {
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        // Nobody states the domain.
        let never = one_result(at(0), at(0), vec![stage(&[0], &[])]);
        assert_eq!(never.check().err(), Some(late));
        // A stage cannot settle the capacity of storage it is already writing.
        let same = one_result(at(0), at(0), vec![stage(&[0], &[0])]);
        assert_eq!(same.check().err(), Some(late));
        // One stage measures, the next fills.
        let measured = one_result(at(0), at(0), vec![stage(&[], &[0]), stage(&[0], &[])]);
        assert!(measured.check().is_ok());
        // No Array is sized from its own contents: a stage cannot determine
        // the domain of a result it is filling, whatever storage it was given.
        let own = one_result(at(0), cells(100), vec![stage(&[0], &[0])]);
        assert_eq!(own.check().err(), Some(late));
    }

    /// A unit whose only result is an Array of a Real and an Int, laid out by
    /// the given parts in the given storage.
    fn particles(parts: Vec<(Scalar, u32, u32)>, storage: Vec<Storage>) -> Unit {
        let mut tables = Tables::default();
        let real = tables.kinds.define(Definition::Real);
        let int = tables.kinds.define(Definition::Int);
        let members = vec![("mass".to_string(), real), ("id".to_string(), int)];
        let particle = tables.kinds.define(Definition::Product(members));
        let kind = tables.kinds.define(Definition::Array {
            element: particle,
            axes: vec![AxisKind::Closed],
        });
        let parts = parts
            .into_iter()
            .map(|(scalar, plane, offset)| {
                part(tables.layouts.define(Layout::Scalar(scalar)), plane, offset)
            })
            .collect();
        let group = tables.layouts.define(Layout::Group(parts));
        let layout = tables.layouts.define(Layout::Array(group));
        let planes: Vec<u32> = (0..storage.len() as u32).collect();
        let port = tables.port(kind, layout, vec![cells(4)], &planes);
        Unit {
            outputs: vec![port],
            stages: vec![stage(&planes, &[])],
            storage,
            ..tables.unit()
        }
    }

    #[test]
    fn a_fiber_array_may_be_one_plane_of_whole_fibers_or_one_plane_per_component() {
        // Both scalars in one 16-byte element.
        let together = vec![(Scalar::F64, 0, 0), (Scalar::I32, 0, 8)];
        assert!(particles(together.clone(), vec![out(16)]).check().is_ok());
        // One plane per component.
        let apart = vec![(Scalar::F64, 0, 0), (Scalar::I32, 1, 0)];
        assert!(particles(apart.clone(), vec![out(8), out(4)])
            .check()
            .is_ok());

        // The Int placed on top of the Real.
        let clash = vec![(Scalar::F64, 0, 0), (Scalar::I32, 0, 4)];
        assert!(matches!(
            particles(clash, vec![out(16)]).check().err(),
            Some(Defect::Layout(LayoutDefect::PartsOverlap { .. }))
        ));
        // Twelve bytes of fiber in an eight-byte element.
        let outside = Defect::OutsideElement {
            port: RESULT,
            plane: 0,
        };
        assert_eq!(
            particles(together, vec![out(8)]).check().err(),
            Some(outside)
        );
        // A Real stored as an integer.
        let mismatch = Some(Defect::KindMismatch(RESULT));
        let wrong = vec![(Scalar::I64, 0, 0), (Scalar::I32, 0, 8)];
        assert_eq!(particles(wrong, vec![out(16)]).check().err(), mismatch);
        // Two planes laid out and one slot given.
        let mut short = particles(apart, vec![out(8), out(4)]);
        short.outputs[0].planes.pop();
        assert_eq!(short.check().err(), mismatch);
    }

    #[test]
    fn two_planes_of_one_value_cannot_share_storage() {
        let apart = vec![(Scalar::F64, 0, 0), (Scalar::I32, 1, 0)];
        let mut unit = particles(apart, vec![out(8), out(8)]);
        unit.outputs[0].planes = vec![Slot(0), Slot(0)];
        let repeated = Defect::PlaneRepeated {
            port: RESULT,
            plane: 1,
        };
        assert_eq!(unit.check().err(), Some(repeated));
    }

    #[test]
    fn a_bool_is_stored_as_a_32_bit_integer() {
        let stored = |scalar| {
            let mut tables = Tables::default();
            let (kind, layout) = tables.array(Definition::Bool, scalar);
            let port = tables.port(kind, layout, vec![cells(4)], &[0]);
            Unit {
                outputs: vec![port],
                storage: vec![out(8)],
                stages: vec![stage(&[0], &[])],
                ..tables.unit()
            }
        };
        assert!(stored(Scalar::I32).check().is_ok());
        let mismatch = Some(Defect::KindMismatch(RESULT));
        assert_eq!(stored(Scalar::I64).check().err(), mismatch);
        assert_eq!(stored(Scalar::F32).check().err(), mismatch);
    }

    /// A unit whose only result is an Optional Array of Reals: presence in
    /// shape field 0 and the domain in field 1, storage in slot 0.
    fn optional(
        nest: bool,
        shape: Vec<ShapeEntry>,
        capacity: ShapeEntry,
        stages: Vec<Stage>,
    ) -> Unit {
        let mut tables = Tables::default();
        let (array, array_layout) = tables.reals();
        let mut kind = tables.kinds.define(Definition::Optional(array));
        if nest {
            kind = tables.kinds.define(Definition::Optional(kind));
        }
        let layout = tables.layouts.define(Layout::Optional(array_layout));
        let port = tables.port(kind, layout, shape, &[0]);
        Unit {
            outputs: vec![port],
            storage: vec![Storage::Output(need(8, vec![capacity]))],
            shape_fields: vec![ShapeField::Presence, ShapeField::Domain { rank: 1 }],
            stages,
            ..tables.unit()
        }
    }

    #[test]
    fn an_optional_result_reports_presence_in_the_shape_record() {
        let measure_then_fill = || vec![stage(&[], &[0, 1]), stage(&[0], &[])];
        let unit = optional(false, vec![at(0), at(1)], at(1), measure_then_fill());
        assert!(unit.check().is_ok());

        // Presence is not a size, so it cannot state how much storage to provide.
        let presence = ShapeEntry::Fixed(ShapeValue::Presence(true));
        let unit = optional(false, vec![at(0), at(1)], presence, measure_then_fill());
        assert_eq!(unit.check().err(), Some(Defect::CapacityNotASize(Slot(0))));

        // An Optional never directly holds an Optional.
        let unit = optional(true, vec![at(0), at(1)], cells(100), measure_then_fill());
        let defect = KindDefect::NestedOptional { at: KindRef(3) };
        assert_eq!(unit.check().err(), Some(Defect::Kind(defect)));
    }

    #[test]
    fn a_result_fixed_as_absent_needs_no_sizes() {
        // Nothing determines the domain and nothing writes the storage.
        let absent = ShapeEntry::Fixed(ShapeValue::Presence(false));
        let unit = optional(false, vec![absent, at(1)], cells(0), Vec::new());
        assert!(unit.check().is_ok());

        // A result that may be present owes its domain.
        let unit = optional(false, vec![at(0), at(1)], cells(0), vec![stage(&[], &[0])]);
        assert_eq!(
            unit.check().err(),
            Some(Defect::ResultShapeUnknown(ShapeRef(1)))
        );
    }

    /// A unit whose only result is a Struct of two Arrays of Reals. The
    /// first has a fixed domain and sits in plane 0; the second's domain is
    /// shape field 0 and it sits where `second` says, as a plane and an
    /// offset.
    fn pair(stages: Vec<Stage>, second: (u32, u32)) -> Unit {
        let mut tables = Tables::default();
        let (array, array_layout) = tables.reals();
        let members = vec![("first".to_string(), array), ("second".to_string(), array)];
        let kind = tables.kinds.define(Definition::Struct(members));
        let parts = vec![
            part(array_layout, 0, 0),
            part(array_layout, second.0, second.1),
        ];
        let layout = tables.layouts.define(Layout::Group(parts));
        let count = second.0 as usize + 1;
        let port = Port {
            planes: slots(count),
            ..tables.port(kind, layout, vec![cells(4), at(0)], &[])
        };
        Unit {
            outputs: vec![port],
            storage: vec![out(16), Storage::Output(need(8, vec![at(0)]))][..count].to_vec(),
            shape_fields: vec![ShapeField::Domain { rank: 1 }],
            stages,
            ..tables.unit()
        }
    }

    #[test]
    fn a_struct_may_fill_one_array_then_size_and_fill_the_other() {
        // Fill the first, work out the second's domain from it, fill the second.
        let staged = vec![stage(&[0], &[]), stage(&[], &[0]), stage(&[1], &[])];
        assert!(pair(staged, (1, 0)).check().is_ok());

        // Filling the second before its domain is known is still refused.
        let early = vec![stage(&[0, 1], &[]), stage(&[], &[0])];
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        assert_eq!(pair(early, (1, 0)).check().err(), Some(late));

        // Two Arrays with different domains cannot be kept side by side in the
        // elements of one plane.
        let shared = Defect::PlaneSharedAcrossParts {
            port: RESULT,
            plane: 0,
        };
        assert_eq!(pair(Vec::new(), (0, 8)).check().err(), Some(shared));
    }

    #[test]
    fn every_result_sharing_storage_must_be_sized_before_it_is_written() {
        // Two results in one slot. The first has a fixed domain; the second's
        // is determined by the very stage that writes the slot.
        let mut tables = Tables::default();
        let (kind, layout) = tables.reals();
        let fixed = tables.port(kind, layout, vec![cells(4)], &[0]);
        let late = tables.port(kind, layout, vec![at(0)], &[0]);
        let unit = Unit {
            outputs: vec![fixed, late],
            storage: vec![out(8)],
            shape_fields: vec![ShapeField::Domain { rank: 1 }],
            stages: vec![stage(&[0], &[0])],
            ..tables.unit()
        };
        let defect = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        assert_eq!(unit.check().err(), Some(defect));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_is_refused_as_unsupported() {
        let mut tables = Tables::default();
        let matrix = Definition::Mat {
            rows: 3,
            columns: 3,
        };
        let (kind, layout) = tables.array(matrix, Scalar::F64);
        let port = tables.port(kind, layout, vec![cells(4)], &[0]);
        let unit = Unit {
            outputs: vec![port],
            storage: vec![out(72)],
            stages: vec![stage(&[0], &[])],
            ..tables.unit()
        };
        assert_eq!(unit.check().err(), Some(Defect::KindUnsupported(RESULT)));
    }

    #[test]
    fn a_deeply_shared_kind_is_checked_without_walking_every_scalar() {
        let mut tables = Tables::default();
        let mut kind = tables.kinds.define(Definition::Real);
        for _ in 0..40 {
            kind = tables.kinds.define(Definition::Tuple(vec![kind, kind]));
        }
        let layout = tables.layouts.define(Layout::Scalar(Scalar::F64));
        let port = tables.port(kind, layout, Vec::new(), &[0]);
        let unit = Unit {
            outputs: vec![port],
            storage: vec![out(8)],
            ..tables.unit()
        };
        // 2^40 scalars against a layout of one: refused at once.
        assert_eq!(unit.check().err(), Some(Defect::KindMismatch(RESULT)));
    }

    #[test]
    fn an_empty_domain_counts_zero_whatever_the_axis_order() {
        let domain = |extents: &[u64]| {
            let axis = |extent: &u64| Axis {
                origin: 0,
                extent: *extent,
            };
            ShapeValue::Domain(extents.iter().map(axis).collect())
        };
        assert_eq!(domain(&[0, u64::MAX, 2]).count(), Some(0));
        assert_eq!(domain(&[u64::MAX, 2, 0]).count(), Some(0));
    }

    /// A unit that computes nothing: `current = state ?? initial`, with the
    /// State an Optional Array and the initial value an Array stored as the
    /// given scalar.
    fn current(initial: Scalar, selection: impl Fn(KindRef, KindRef) -> Selection) -> Unit {
        let mut tables = Tables::default();
        let (array, array_layout) = tables.reals();
        let optional = tables.kinds.define(Definition::Optional(array));
        let optional_layout = tables.layouts.define(Layout::Optional(array_layout));
        let (_, initial_layout) = tables.array(Definition::Real, initial);
        let present = ShapeEntry::Runtime(ShapeRef(0));
        let state = tables.port(optional, optional_layout, vec![present, at(1)], &[0]);
        let fallback = tables.port(array, initial_layout, vec![at(2)], &[1]);
        Unit {
            inputs: vec![state, fallback],
            selected: vec![selection(array, optional)],
            storage: vec![
                Storage::Input(need(8, vec![at(1)])),
                Storage::Input(need(8, vec![at(2)])),
            ],
            shape_fields: vec![
                ShapeField::Presence,
                ShapeField::Domain { rank: 1 },
                ShapeField::Domain { rank: 1 },
            ],
            ..tables.unit()
        }
    }

    #[test]
    fn a_result_may_be_an_optional_input_or_a_fallback_without_a_copy() {
        let choose = |kind, first, otherwise| Selection {
            root: STATE,
            kind,
            first,
            otherwise,
        };
        let selected = PortRef::Selected(0);
        let mismatch = Some(Defect::KindMismatch(selected));

        // The State when present, the initial value otherwise. Nothing is
        // computed, so the unit has no stages and no storage of its own.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(1)));
        assert!(unit.check().is_ok());

        // The State as it stands, present or not.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, None));
        assert!(unit.check().is_ok());

        // The two must be kept the same way, so the result has one layout.
        let unit = current(Scalar::F32, |array, _| choose(array, 0, Some(1)));
        assert_eq!(unit.check().err(), mismatch);

        // The first of two must be an Optional: the initial value never is.
        let unit = current(Scalar::F64, |array, _| choose(array, 1, Some(0)));
        assert_eq!(unit.check().err(), mismatch);

        // The result is of the fallback's kind, not the Optional's.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, Some(1)));
        assert_eq!(unit.check().err(), mismatch);

        // A selection must name inputs the unit has.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(7)));
        assert_eq!(unit.check().err(), Some(Defect::UnknownInput(selected)));
    }

    #[test]
    fn a_result_may_be_smaller_than_the_storage_stated_for_it() {
        // An Array read from slot 0, whose domain is shape field 0, and a
        // result in slot 1 whose domain, field 1, a first stage determines.
        // The result's storage is stated by the input's size, not its own,
        // as for a filter that keeps at most as many elements as it reads.
        // Nothing the code computes is needed to provide that storage. The
        // check does not show that the result fits.
        let mut tables = Tables::default();
        let (kind, layout) = tables.reals();
        let input = tables.port(kind, layout, vec![at(0)], &[0]);
        let output = tables.port(kind, layout, vec![at(1)], &[1]);
        let reading = |stage: Stage| Stage {
            reads: vec![Slot(0)],
            ..stage
        };
        let unit = Unit {
            inputs: vec![input],
            outputs: vec![output],
            storage: vec![
                Storage::Input(need(8, vec![at(0)])),
                Storage::Output(need(8, vec![at(0)])),
            ],
            shape_fields: vec![ShapeField::Domain { rank: 1 }; 2],
            stages: vec![reading(stage(&[], &[1])), reading(stage(&[1], &[]))],
            ..tables.unit()
        };
        assert!(unit.check().is_ok());
    }
}
