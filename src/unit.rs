//! The description of one piece of compiled code.

use crate::kind::{Definition, KindDefect, KindRef, Kinds, ShapeField};
use crate::layout::{Layout, LayoutDefect, LayoutRef, Layouts, Measures, Plane};
use crate::program::{Demand, Node};

/// One axis of an Array domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub origin: i64,
    pub extent: u64,
}

/// One fact about a value that is only known at run time: the length of a
/// List, the domain of an Array, or whether an Optional is present.
///
/// A shape value is logical. It says how many elements a value has and where,
/// or whether the value is there at all. It is not a number of bytes. The
/// bytes of one element are fixed at compile time, and the bytes of an
/// allocation are never stated in a unit: the runtime works them out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeValue {
    /// Whether an Optional value is present. It counts nothing.
    Presence(bool),
    Length(u64),
    Domain(Vec<Axis>),
}

impl ShapeValue {
    /// How many elements a length or a domain counts, or nothing if this is
    /// presence or the count does not fit in 64 bits.
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

    /// Whether this says an Optional value is absent. An absent value has no
    /// storage and no other shape values.
    pub fn is_absent(&self) -> bool {
        matches!(self, ShapeValue::Presence(false))
    }

    /// Whether this is a length or a domain that counts no elements. An
    /// empty value is present; an absent one is not empty.
    pub fn is_empty(&self) -> bool {
        self.count() == Some(0)
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

/// One shape value in a unit's table of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeRef(pub u32);

/// A shape value that is either fixed in the code or named in the unit's
/// table.
///
/// A shape value belongs to the value it describes and is kept apart from
/// that value's planes. An absent value has a presence and no storage, and a
/// value that is kept keeps its shape values with it.
///
/// Shape values are consumed the way storage is: only after they are
/// produced. Unlike storage, which several stages may write in list order, a
/// shape value is fixed once produced, so each named one has one producer
/// and any number of consumers. The shape values of an input come with the
/// value bound to it. Any other is determined by one stage. A stage that
/// uses it, storage whose capacity names it and a result that has it are its
/// consumers. Several ports may refer to one shape value. That states once
/// that the values have the same shape.
///
/// The runtime is a consumer too, but only where something it does needs the
/// value: providing storage whose capacity names it, verifying a
/// specialization, making a selection, or answering an observation. A shape
/// value that only compiled code consumes need not be read by the runtime.
///
/// Before such an operation uses a shape value, the runtime must hold the
/// value that applies to the inputs being used. If it holds no valid one, it
/// obtains it before that operation proceeds. This dependency does not by
/// itself make unrelated work wait.
///
/// A shape value that was never produced is different from a presence that
/// says a value is absent. Producing and consuming apply only where a shape
/// value applies. Those within an Optional that turns out absent are not
/// produced, and a stage that uses them consumes nothing then: a stage that
/// works out a length when present, and zero otherwise, uses the presence
/// and, only when present, the length. Storage provided for a result that
/// turns out absent goes unused. The check works from what the description
/// declares, not from which shape values apply on a given run.
///
/// In an input port, `Fixed` is a specialization: the value bound must have
/// that shape, and the runtime verifies it before any stage runs. A change
/// of extent needs no recompilation unless the code chose to specialize on
/// it. Two inputs that refer to one shape value must agree in the same way.
/// The runtime never treats two different shapes as one.
///
/// Gap: how a shape value is physically kept, in which memory, how it reaches
/// a stage or a later consumer, and how a stage is told that one does not
/// apply, is not written.
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

/// How many elements a slot must be able to hold: the product of the counts
/// of these shape values. Empty means one element. Each is a length or a
/// domain, since presence counts nothing.
///
/// What the code writes must fit. That is an obligation on the code. The
/// check cannot establish it, and the runtime is not required to read a
/// shape value in order to verify it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capacity(pub Vec<ShapeEntry>);

/// What storage a slot needs: which memory, the bytes of one element, which
/// are fixed at compile time, and how many elements.
///
/// A unit never states the bytes of an allocation, and it never asks for
/// storage at all: this description is the request. The runtime provides a
/// [`Region`] before the stage that first writes the slot. That makes the
/// runtime a consumer of every shape value the capacity names, and of no
/// others.
#[derive(Clone, Debug)]
pub struct Need {
    pub memory: Memory,
    /// The bytes of one element.
    pub element_bytes: u32,
    /// The alignment the start of the storage must have, in bytes.
    pub align: u32,
    /// Every shape value here is produced before the stage that first writes
    /// the slot.
    pub capacity: Capacity,
}

/// The storage a slot is given at run time: part or all of an allocation,
/// passed to a stage as a base and an offset from that base. A whole
/// allocation is the case where the offset is zero.
///
/// The region starts where the slot's alignment allows and holds the stated
/// capacity. Code may not assume that a slot begins an allocation or that
/// two slots lie in different ones, and it reads and writes nothing outside
/// the regions it was given. The regions of different slots do not overlap,
/// except where one slot is an alias.
///
/// Gap: what a base is differs by target and is not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region<Base> {
    pub base: Base,
    /// Bytes from the base to the start of the region.
    pub offset: u64,
}

/// What each slot is for. This decides who may write it.
#[derive(Clone, Debug)]
pub enum Storage {
    /// A plane of an input value: the memory, the bytes of one element and
    /// the number of elements the unit expects to find. Never written.
    Input(Need),
    /// A plane of a result. Written by this unit, then handed to the runtime.
    Output(Need),
    /// A plane of a computed result that is an input plane unchanged. Shares
    /// its storage. A result that is a whole input value is a `Selection`.
    Alias(Slot),
    /// Private to one run of the unit. It has no identity outside it.
    Scratch(Need),
}

/// One value a unit takes in: the value of a node, with its kind, its
/// layout, its shape and the storage that holds it.
///
/// The kind says what the value is and the layout says how it is kept. The
/// layout must lay out that kind, and the port gives one shape value for each
/// run-time fact of the kind and one slot for each plane of the layout.
///
/// Decide: a value is bound to a port only if their layouts are identical.
/// Converting between layouts would be an explicit computation, never
/// something the runtime does silently while binding.
#[derive(Clone, Debug)]
pub struct InputPort {
    /// The node whose value this is.
    pub node: Node,
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

/// One value a unit computes: the result that meets one demand. Its kind,
/// layout, shape and planes are stated as for an [`InputPort`].
#[derive(Clone, Debug)]
pub struct OutputPort {
    /// The demand this result meets.
    pub demand: Demand,
    pub kind: KindRef,
    pub layout: LayoutRef,
    pub shape: Vec<ShapeEntry>,
    pub planes: Vec<Slot>,
}

/// A result that is one of the unit's inputs, unchanged.
///
/// The result is the value itself: its shape, its layout and its storage.
/// Nothing is computed and nothing is copied.
///
/// With no `otherwise`, the result is the input `first`. This is how a unit
/// answers with a State as it stands.
///
/// With an `otherwise`, `first` is an Optional input. When it is present the
/// result is what it holds, and when it is absent the result is the input
/// `otherwise`. The runtime makes the choice, which makes it a consumer of
/// the presence of `first`. The two must be kept the same way, so the result
/// has one layout whichever is chosen. They need not have the same shape:
/// the result has the shape of the one chosen.
///
/// An input that the unit uses only as the `otherwise` of selections is
/// needed only when one of them would choose it. While the Optional input is
/// present the runtime need not hold it at all.
///
/// Gap: choosing on a device, or demanding the fallback conditionally there,
/// is not designed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The demand this result meets.
    pub demand: Demand,
    /// A reference into the unit's table of kinds.
    pub kind: KindRef,
    /// Which input is chosen: always, or when it is present.
    pub first: u32,
    /// Which input is chosen when `first` is absent.
    pub otherwise: Option<u32>,
}

/// One compiled computation: an entry point in the code. A stage is one
/// launch. The runtime starts it, and it runs to completion without the
/// runtime. A stage is the only execution boundary in this contract.
///
/// A stage is given what it consumes and the storage it writes. It computes
/// and produces results: what it writes, and the shape values it determines.
/// Compiled code never calls the runtime. Everything a stage needs is
/// provided before it runs, and whenever the runtime must act, it acts
/// between stages. All providing of storage, and any waiting for it, is the
/// runtime's.
///
/// The runtime starts every stage, and between two stages it need do nothing
/// more. It provides storage first only if the next stage writes storage not
/// yet provided, and it waits for an earlier stage only if it must itself
/// read something that stage produced. Computations that can run as one
/// launch may be one stage; nothing requires it.
///
/// A stage consumes only what was produced before it, by the values bound to
/// inputs or by earlier stages. That holds for shape values exactly as it
/// holds for storage.
///
/// No value takes its shape from its own contents. The shape of a result may
/// depend on data, but only on data computed before the result is filled. A
/// stage that writes part of a result uses the shape values that govern that
/// part, so it cannot be the stage that determines them. One stage
/// determines, and a later stage fills.
///
/// For fixed code, what a stage produces depends only on what it consumes,
/// and its description lists all of that. A stage that depends on what
/// storage held before the stage wrote it lists that storage among what it
/// reads. A runtime that holds a shape value produced from the same consumed
/// values may use it without waiting for the stage to produce it again. That
/// does not by itself let the runtime leave the stage out, since whatever
/// else the stage writes is still owed. Where everything a stage produces is
/// already held and valid, the stage need not run. How a runtime knows that
/// consumed values are unchanged is its own business. Treating any write as
/// a change is always sound.
///
/// Stages are listed in an order in which they can run. The runtime owns the
/// order of execution. It may run stages in another order, or overlapped,
/// only where each still finds in storage, and in the shape values it uses,
/// what list order would have given it.
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
/// Each slot is passed to a stage as a [`Region`].
///
/// Gap: nothing says where a stage runs. The code is emitted for one target,
/// so a stage whose inputs are all available on the host cannot be placed
/// there.
///
/// Gap: which of the shape values a stage uses set the extent it is
/// dispatched over is not stated.
///
/// Gap: the rest of the calling convention of a stage on each target is not
/// written yet: what a base is, how the entry is named, and how shape values
/// are passed.
#[derive(Clone, Debug, Default)]
pub struct Stage {
    /// Storage this stage reads.
    pub reads: Vec<Slot>,
    /// Storage this stage writes.
    pub writes: Vec<Slot>,
    /// Shape values this stage consumes. This declares a dependency. It does
    /// not require the code to load the value, and it does not by itself
    /// require the runtime to read it.
    pub uses: Vec<ShapeRef>,
    /// Shape values this stage produces.
    pub determines: Vec<ShapeRef>,
}

/// Everything the runtime needs to know to run a piece of compiled code.
///
/// A unit is a description, not an execution boundary. It is the scope in
/// which storage, shape values, kinds and layouts are stated once, and it
/// says which results meet which demands. Its stages are what runs. One rule
/// holds inside a unit and between units: whatever is consumed was produced
/// first, by a stage or by whatever produced the value bound to an input.
///
/// A unit's results are a function of its inputs and of nothing else.
///
/// The job of a unit is to supply values at requested sites of indexed
/// values. The values it reads and the values it produces are held in
/// memory. That does not require a fresh copy; see `Storage::Alias`.
///
/// A value's domain is where it has values at all. It is distinct from the
/// sites a unit is asked to fill on one run, and from the sites that storage
/// currently covers. A domain in a port is the value's domain.
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
    pub inputs: Vec<InputPort>,
    /// Results the unit computes.
    pub outputs: Vec<OutputPort>,
    /// Results that are inputs, unchanged.
    pub selected: Vec<Selection>,
    pub storage: Vec<Storage>,
    /// Every shape value not fixed at compile time, each stated once.
    pub shape_fields: Vec<ShapeField>,
    /// In list order: an order in which the stages can run.
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
    /// A stage uses a shape value, or writes storage whose capacity names
    /// one, that nothing before it produces. A stage cannot produce what it
    /// consumes, so it cannot give a result it is filling its shape.
    ShapeNotYetKnown {
        stage: u32,
        field: ShapeRef,
    },
    /// A shape value has more than one producer: two stages, or a stage and
    /// an input.
    ShapeProducedTwice(ShapeRef),
    /// A stage writes part of a result without using a shape value that
    /// governs that part.
    GoverningShapeUnused {
        stage: u32,
        field: ShapeRef,
    },
    /// A shape value of a result has no producer.
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
    /// kind, it gives the wrong number of shape values or of slots, or a
    /// shape value is of the wrong sort.
    KindMismatch(PortRef),
    /// A selection names an input the unit does not have.
    UnknownInput(PortRef),
    /// A storage capacity is stated as presence, which counts nothing.
    CapacityNotACount(Slot),
    /// Two planes of one value are given the same storage.
    PlaneRepeated {
        port: PortRef,
        plane: Plane,
    },
    /// A plane's layout spans more bytes than one element of its storage
    /// holds.
    OutsideElement {
        port: PortRef,
        plane: Plane,
    },
    /// Parts of a value with different shape values are placed in one
    /// plane. A plane holds one run of elements, so everything in it shares
    /// one domain and one presence.
    PlaneSharedAcrossParts {
        port: PortRef,
        plane: Plane,
    },
}

/// A unit description that passed [`Unit::check`].
///
/// The field is private, so holding one is proof the description passed these
/// checks. The tables of kinds and of layouts are well formed and state each
/// entry once. Every slot and shape field named exists. Each port's kind has
/// its storage described, its layout lays out that kind, and it gives one
/// shape value for each run-time fact and one slot for each plane. A
/// selection chooses among inputs of the right kinds that are kept one way. No
/// two planes of a value share storage, and each plane's layout fits inside
/// one element of its storage. No capacity is stated as presence. No stage
/// writes an input or an alias, and an alias refers to an input. Each shape
/// value has at most one producer. A stage uses only what was produced
/// before it, and a stage that writes part of a result uses the shape values
/// that govern that part. Every shape value a capacity names is produced
/// before the stage that first writes the storage. Every shape value of a
/// result has a producer, except those of a part fixed as absent, which do
/// not apply.
///
/// Passing these checks does not make a unit ready to run. They do not show
/// that every result is written by some stage, that storage is read only
/// after something wrote it, that what the code writes fits the capacity
/// stated for it, or that an alignment is usable, and they cannot show that
/// the code honors the description. They establish an order within the
/// description. They say nothing about where a shape value is at run time
/// or whether the work that produces it has completed.
///
/// Decide: how far the runtime trusts code from a supplier it did not build.
/// Nothing in this contract confines code to the storage it declared.
#[derive(Clone, Debug)]
pub struct Checked {
    unit: Unit,
    governs: Vec<Vec<ShapeRef>>,
}

impl Checked {
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    /// The shape values that govern what a slot holds of a result. A domain
    /// governs the planes of its Array, and a presence governs everything
    /// its Optional holds. A stage that writes the slot must use each of
    /// them. Empty where the slot holds no part of a result, or a part
    /// whose shape is fixed.
    pub fn governing(&self, slot: Slot) -> &[ShapeRef] {
        self.governs.get(slot.0 as usize).map_or(&[], Vec::as_slice)
    }

    /// What produces a shape value, or nothing if the description gives it
    /// no producer. A checked unit never gives one two.
    pub fn producer(&self, shape: ShapeRef) -> Option<Producer> {
        let named = ShapeEntry::Runtime(shape);
        let brings = |input: &InputPort| input.shape.contains(&named);
        let determines = |stage: &Stage| stage.determines.contains(&shape);
        let input = self.unit.inputs.iter().position(brings);
        let stage = self.unit.stages.iter().position(determines);
        let input = input.map(|input| Producer::Input(input as u32));
        input.or(stage.map(|stage| Producer::Stage(stage as u32)))
    }

    /// Everything the description says depends on a shape value.
    pub fn consumers(&self, shape: ShapeRef) -> Vec<Consumer> {
        let unit = &self.unit;
        let named = ShapeEntry::Runtime(shape);
        let mut consumers = Vec::new();
        for (index, stage) in unit.stages.iter().enumerate() {
            if stage.uses.contains(&shape) {
                consumers.push(Consumer::Stage(index as u32));
            }
        }
        for (index, storage) in unit.storage.iter().enumerate() {
            if let Storage::Output(need) | Storage::Scratch(need) = storage {
                if need.capacity.0.contains(&named) {
                    consumers.push(Consumer::Capacity(Slot(index as u32)));
                }
            }
        }
        for (index, output) in unit.outputs.iter().enumerate() {
            if output.shape.contains(&named) {
                consumers.push(Consumer::Result(index as u32));
            }
        }
        for (index, selection) in unit.selected.iter().enumerate() {
            // An Optional's presence is the first of its shape values.
            let first = &unit.inputs[selection.first as usize];
            if selection.otherwise.is_some() && first.shape.first() == Some(&named) {
                consumers.push(Consumer::Selection(index as u32));
            }
        }
        consumers
    }
}

/// What fixes a shape value. Each shape value has exactly one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// It comes with the value bound to this input. Where several inputs
    /// refer to one shape value, this is the first of them.
    Input(u32),
    /// This stage determines it.
    Stage(u32),
}

/// Something that depends on a shape value. A shape value may have any
/// number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    /// This stage uses it.
    Stage(u32),
    /// The capacity of this storage names it, so the runtime consumes it to
    /// provide the storage.
    Capacity(Slot),
    /// This computed result has it.
    Result(u32),
    /// The runtime chooses this selection by it.
    Selection(u32),
}

/// What an input port and an output port state alike.
struct Stated<'a> {
    at: PortRef,
    kind: KindRef,
    layout: LayoutRef,
    shape: &'a [ShapeEntry],
    planes: &'a [Slot],
}

/// Which shape values govern each plane of one port.
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
                    self.walk(part.layout, plane + part.plane.0, next, enclosing, absent)?;
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
                    for entry in &need.capacity.0 {
                        exists(entry)?;
                        let presence = match entry {
                            ShapeEntry::Fixed(value) => matches!(value, ShapeValue::Presence(_)),
                            ShapeEntry::Runtime(field) => {
                                self.shape_fields[field.0 as usize] == ShapeField::Presence
                            }
                        };
                        if presence {
                            return Err(Defect::CapacityNotACount(Slot(index as u32)));
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

        // A shape value is produced by the value bound to an input or by one
        // stage.
        let mut produced = vec![false; self.shape_fields.len()];
        // The shape values that govern what each slot holds of a result, and
        // the shape values results have.
        let mut governs: Vec<Vec<ShapeRef>> = vec![Vec::new(); self.storage.len()];
        let mut owed: Vec<ShapeRef> = Vec::new();

        let inputs = self.inputs.iter().enumerate().map(|(index, port)| Stated {
            at: PortRef::Input(index as u32),
            kind: port.kind,
            layout: port.layout,
            shape: &port.shape,
            planes: &port.planes,
        });
        let outputs = self.outputs.iter().enumerate().map(|(index, port)| Stated {
            at: PortRef::Output(index as u32),
            kind: port.kind,
            layout: port.layout,
            shape: &port.shape,
            planes: &port.planes,
        });
        for port in inputs.chain(outputs) {
            let at = port.at;
            let input = matches!(at, PortRef::Input(_));

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
            for (field, entry) in self.kinds.shape_fields(port.kind).iter().zip(port.shape) {
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
                    return Err(Defect::PlaneRepeated {
                        port: at,
                        plane: Plane(plane),
                    });
                }
                used.push(backing);
                if measures.spans[layout][plane as usize].1 > need.element_bytes {
                    return Err(Defect::OutsideElement {
                        port: at,
                        plane: Plane(plane),
                    });
                }
            }

            // Which shape values govern each plane.
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
                .map_err(|plane| Defect::PlaneSharedAcrossParts {
                    port: at,
                    plane: Plane(plane),
                })?;
            let runtime = |fact: usize| match &port.shape[fact] {
                ShapeEntry::Runtime(field) => Some(*field),
                ShapeEntry::Fixed(_) => None,
            };
            if input {
                for field in (0..port.shape.len()).filter_map(runtime) {
                    produced[field.0 as usize] = true;
                }
                continue;
            }
            for (plane, slot) in port.planes.iter().enumerate() {
                if !governed.absent_planes[plane] {
                    let over = governed.over[plane].iter().flatten();
                    governs[slot.0 as usize].extend(over.filter_map(|fact| runtime(*fact)));
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

        // A stage consumes only what was produced before it, and each shape
        // value has one producer.
        let mut provided = vec![false; self.storage.len()];
        for (index, stage) in self.stages.iter().enumerate() {
            let index = index as u32;
            let late = |field: ShapeRef| Defect::ShapeNotYetKnown {
                stage: index,
                field,
            };
            let place = |field: &ShapeRef| match field.0 as usize {
                place if place < self.shape_fields.len() => Ok(place),
                _ => Err(Defect::UnknownShapeField(*field)),
            };
            for field in &stage.uses {
                if !produced[place(field)?] {
                    return Err(late(*field));
                }
            }
            for slot in &stage.reads {
                storage(*slot)?;
            }
            for slot in &stage.writes {
                let (Storage::Output(need) | Storage::Scratch(need)) = storage(*slot)? else {
                    return Err(Defect::WritesShared {
                        stage: index,
                        slot: *slot,
                    });
                };
                // Providing the storage consumes what its capacity names.
                if !std::mem::replace(&mut provided[slot.0 as usize], true) {
                    for entry in &need.capacity.0 {
                        match entry {
                            ShapeEntry::Runtime(field) if !produced[field.0 as usize] => {
                                return Err(late(*field));
                            }
                            _ => {}
                        }
                    }
                }
                // Filling part of a result consumes what governs it.
                for field in &governs[slot.0 as usize] {
                    if !stage.uses.contains(field) {
                        return Err(Defect::GoverningShapeUnused {
                            stage: index,
                            field: *field,
                        });
                    }
                }
            }
            for field in &stage.determines {
                if std::mem::replace(&mut produced[place(field)?], true) {
                    return Err(Defect::ShapeProducedTwice(*field));
                }
            }
        }
        match owed.into_iter().find(|field| !produced[field.0 as usize]) {
            Some(field) => Err(Defect::ResultShapeUnknown(field)),
            None => Ok(Checked {
                unit: self,
                governs,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::AxisKind;
    use crate::layout::{Part, Scalar};
    use crate::program::StateId;

    const NODE: Node = Node::State(StateId(0));
    const STATE: Demand = Demand::Value(NODE);
    const RESULT: PortRef = PortRef::Output(0);

    fn need(element_bytes: u32, capacity: Vec<ShapeEntry>) -> Need {
        Need {
            memory: Memory::Device(0),
            element_bytes,
            align: 8,
            capacity: Capacity(capacity),
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

    /// A stage that writes the given slots, uses the given shape values and
    /// determines the given shape values.
    fn stage(writes: &[u32], uses: &[u32], determines: &[u32]) -> Stage {
        let fields = |fields: &[u32]| fields.iter().map(|field| ShapeRef(*field)).collect();
        Stage {
            reads: Vec::new(),
            writes: writes.iter().map(|slot| Slot(*slot)).collect(),
            uses: fields(uses),
            determines: fields(determines),
        }
    }

    fn part(layout: LayoutRef, plane: u32, offset: u32) -> Part {
        Part {
            layout,
            plane: Plane(plane),
            offset,
        }
    }

    fn slots(count: usize) -> Vec<Slot> {
        (0..count as u32).map(Slot).collect()
    }

    type Parts = (KindRef, LayoutRef, Vec<ShapeEntry>, Vec<Slot>);

    impl From<Parts> for InputPort {
        fn from((kind, layout, shape, planes): Parts) -> Self {
            InputPort {
                node: NODE,
                kind,
                layout,
                shape,
                planes,
            }
        }
    }

    impl From<Parts> for OutputPort {
        fn from((kind, layout, shape, planes): Parts) -> Self {
            OutputPort {
                demand: STATE,
                kind,
                layout,
                shape,
                planes,
            }
        }
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

        /// A port for the State, as an input or an output.
        fn port<P: From<Parts>>(
            &self,
            kind: KindRef,
            layout: LayoutRef,
            shape: Vec<ShapeEntry>,
            planes: &[u32],
        ) -> P {
            let planes = planes.iter().map(|slot| Slot(*slot)).collect();
            P::from((kind, layout, shape, planes))
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
            stages: vec![stage(&[0], &[], &[])],
            ..tables.unit()
        };
        let defect = Defect::WritesShared {
            stage: 0,
            slot: Slot(0),
        };
        assert_eq!(unit.check().err(), Some(defect));
    }

    #[test]
    fn a_stage_consumes_only_what_was_produced_before_it() {
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        // Nothing produces the domain.
        let never = one_result(at(0), at(0), vec![stage(&[0], &[0], &[])]);
        assert_eq!(never.check().err(), Some(late));
        // A stage cannot produce the capacity of storage it is writing.
        let same = one_result(at(0), at(0), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(same.check().err(), Some(late));
        // One stage determines the domain and the next fills.
        let stages = vec![stage(&[], &[], &[0]), stage(&[0], &[0], &[])];
        assert!(one_result(at(0), at(0), stages).check().is_ok());

        // No Array takes its shape from its own contents. A stage that fills a
        // result uses its domain, so it cannot also determine it, whatever
        // storage it was given.
        let own = one_result(at(0), cells(100), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(own.check().err(), Some(late));
        // Nor may it leave the domain out of what it uses.
        let unused = Defect::GoverningShapeUnused {
            stage: 0,
            field: ShapeRef(0),
        };
        let blind = one_result(at(0), cells(100), vec![stage(&[0], &[], &[0])]);
        assert_eq!(blind.check().err(), Some(unused));
    }

    #[test]
    fn several_ports_may_share_a_shape_value_but_it_has_one_producer() {
        // An input Array and a Struct of two Arrays, all with the domain in
        // shape field 0. The input produces it; nothing restates it.
        let shared = |stages: Vec<Stage>| {
            let mut tables = Tables::default();
            let (array, array_layout) = tables.reals();
            let members = vec![("left".to_string(), array), ("right".to_string(), array)];
            let pair = tables.kinds.define(Definition::Struct(members));
            let parts = vec![part(array_layout, 0, 0), part(array_layout, 1, 0)];
            let pair_layout = tables.layouts.define(Layout::Group(parts));
            let input = tables.port(array, array_layout, vec![at(0)], &[0]);
            let output = tables.port(pair, pair_layout, vec![at(0), at(0)], &[1, 2]);
            Unit {
                inputs: vec![input],
                outputs: vec![output],
                storage: vec![
                    Storage::Input(need(8, vec![at(0)])),
                    Storage::Output(need(8, vec![at(0)])),
                    Storage::Output(need(8, vec![at(0)])),
                ],
                shape_fields: vec![ShapeField::Domain { rank: 1 }; 2],
                stages,
                ..tables.unit()
            }
        };
        assert!(shared(vec![stage(&[1, 2], &[0], &[])]).check().is_ok());

        // A stage may not determine a shape value an input brings.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(0)));
        let again = vec![stage(&[], &[], &[0]), stage(&[1, 2], &[0], &[])];
        assert_eq!(shared(again).check().err(), twice);
        // Nor may two stages determine one shape value.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(1)));
        let both = vec![stage(&[], &[], &[1]), stage(&[], &[], &[1])];
        assert_eq!(shared(both).check().err(), twice);
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
            stages: vec![stage(&planes, &[], &[])],
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
            plane: Plane(0),
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
            plane: Plane(1),
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
                stages: vec![stage(&[0], &[], &[])],
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
    fn an_optional_result_states_presence_apart_from_its_storage() {
        let measure_then_fill = || vec![stage(&[], &[], &[0, 1]), stage(&[0], &[0, 1], &[])];
        let unit = optional(false, vec![at(0), at(1)], at(1), measure_then_fill());
        assert!(unit.check().is_ok());

        // Presence counts nothing, so it cannot state how much storage to provide.
        let presence = ShapeEntry::Fixed(ShapeValue::Presence(true));
        let unit = optional(false, vec![at(0), at(1)], presence, measure_then_fill());
        assert_eq!(unit.check().err(), Some(Defect::CapacityNotACount(Slot(0))));

        // An Optional never directly holds an Optional.
        let unit = optional(true, vec![at(0), at(1)], cells(100), measure_then_fill());
        let defect = KindDefect::NestedOptional { at: KindRef(3) };
        assert_eq!(unit.check().err(), Some(Defect::Kind(defect)));
    }

    #[test]
    fn a_result_fixed_as_absent_owes_no_shape() {
        // Nothing determines the domain and nothing writes the storage.
        let absent = ShapeEntry::Fixed(ShapeValue::Presence(false));
        let unit = optional(false, vec![absent, at(1)], cells(0), Vec::new());
        assert!(unit.check().is_ok());

        // A result that may be present owes its domain.
        let unit = optional(
            false,
            vec![at(0), at(1)],
            cells(0),
            vec![stage(&[], &[], &[0])],
        );
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
        let port = OutputPort {
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
    fn a_struct_may_fill_one_array_then_shape_and_fill_the_other() {
        // Fill the first, work out the second's domain from it, fill the second.
        let staged = vec![
            stage(&[0], &[], &[]),
            stage(&[], &[], &[0]),
            stage(&[1], &[0], &[]),
        ];
        let staged = pair(staged, (1, 0)).check().unwrap();
        // The second Array's domain governs its plane. The first is fixed.
        assert_eq!(staged.governing(Slot(1)), [ShapeRef(0)]);
        assert_eq!(staged.governing(Slot(0)), []);

        // Filling the second before its domain is produced is refused.
        let early = vec![stage(&[0, 1], &[0], &[]), stage(&[], &[], &[0])];
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        assert_eq!(pair(early, (1, 0)).check().err(), Some(late));

        // Two Arrays with different domains cannot be kept side by side in the
        // elements of one plane.
        let shared = Defect::PlaneSharedAcrossParts {
            port: RESULT,
            plane: Plane(0),
        };
        assert_eq!(pair(Vec::new(), (0, 8)).check().err(), Some(shared));
    }

    #[test]
    fn every_result_sharing_storage_has_its_shape_before_it_is_written() {
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
            stages: vec![stage(&[0], &[0], &[0])],
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
            stages: vec![stage(&[0], &[], &[])],
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

        // Empty is not absent, and absent is not empty.
        let (empty, absent) = (domain(&[4, 0]), ShapeValue::Presence(false));
        assert!(empty.is_empty() && !empty.is_absent());
        assert!(absent.is_absent() && !absent.is_empty());
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
            demand: STATE,
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
    fn capacity_may_be_the_shape_determined_or_a_bound() {
        // An Array read from slot 0, whose domain is shape field 0, and a
        // result in slot 1 whose domain, field 1, a first stage determines
        // and a second stage uses to fill it. This is the shape of a filter,
        // which keeps at most as many elements as it reads.
        let filter = |capacity: ShapeEntry| {
            let mut tables = Tables::default();
            let (kind, layout) = tables.reals();
            let input = tables.port(kind, layout, vec![at(0)], &[0]);
            let output = tables.port(kind, layout, vec![at(1)], &[1]);
            let reading = |stage: Stage| Stage {
                reads: vec![Slot(0)],
                ..stage
            };
            let count = reading(stage(&[], &[0], &[1]));
            let fill = reading(stage(&[1], &[0, 1], &[]));
            Unit {
                inputs: vec![input],
                outputs: vec![output],
                storage: vec![
                    Storage::Input(need(8, vec![at(0)])),
                    Storage::Output(need(8, vec![capacity])),
                ],
                shape_fields: vec![ShapeField::Domain { rank: 1 }; 2],
                stages: vec![count, fill],
                ..tables.unit()
            }
        };
        // Storage for exactly the count: providing it consumes the count.
        let (input, count) = (ShapeRef(0), ShapeRef(1));
        let exact = filter(at(1)).check().unwrap();
        assert_eq!(exact.producer(input), Some(Producer::Input(0)));
        assert_eq!(exact.producer(count), Some(Producer::Stage(0)));
        let stored = Consumer::Capacity(Slot(1));
        let consumers = [Consumer::Stage(1), stored, Consumer::Result(0)];
        assert_eq!(exact.consumers(count), consumers);

        // Storage for as many as the input holds: providing it consumes
        // nothing the code computed. The check does not show that the result
        // fits; that is the code's obligation.
        let bounded = filter(at(0)).check().unwrap();
        assert_eq!(
            bounded.consumers(count),
            [Consumer::Stage(1), Consumer::Result(0)]
        );
    }
}
