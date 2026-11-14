//! The draft contracts between Cartan components.
//!
//! This file holds every declaration: the types each component owns and what
//! it offers the others, in reading order. Each type's documentation is the
//! one definition of its term. Everything is plain description with no
//! dependencies. The `impl`s, including the checks that a description is well
//! formed, are in `checks.rs`, and their tests are in `tests.rs`.
//!
//! There are two layers. The runtime-facing contracts come first: kinds,
//! layouts, programs, units, supply and sessions. They are everything the
//! runtime needs from a compiler and everything an application needs from
//! the runtime, and they use nothing from the second layer, so a compiler
//! built from other components can implement [`Supplier`] and be run by the
//! same runtime. Building with `--no-default-features` compiles this layer
//! alone.
//!
//! The pipeline contracts come last, behind the `pipeline` feature: syntax,
//! graph, code generation and preparation. They are the contracts between
//! the components of the official compiler.
//!
//! A code generator describes each piece of compiled code as a [`Unit`]: the
//! values it reads, the values it produces, the storage it needs and the
//! stages it runs. The runtime owns everything live. It binds inputs,
//! provides storage, starts the stages and publishes results. Compiled code
//! never allocates memory and never publishes.
//!
//! Everything here is a draft. Doc paragraphs starting `Gap:` name parts of a
//! contract that are not written yet; paragraphs starting `Decide:` name open
//! choices.

// ===========================================================================
// Kinds: what a value is
// ===========================================================================
//
// What a value is: its kind.
//
// Kinds are the language's own. This section writes down the ones that cross
// a boundary so that every component names them the same way. The rules that
// give an expression its kind are not here; they belong to analysis.
//
// A kind is logical. It says `Int`, not how many bits an Int has, and it says
// nothing about where a value is stored. Those are choices a port states.
//
// A kind is stated once. Kinds live in a table, and wherever a kind is used
// it is referred to by its place in that table. A Struct nested inside
// another, or a fiber used by many Arrays, appears once however often it is
// used, and its facts are worked out once.
//
// The fibers named here each have one definition. The list is not claimed to
// be complete. Naming a fiber does not oblige any component to support it.
// Where stating how a fiber is stored would be costly, this section declines
// to: it answers "not described" and a component is free to report the
// fiber as unsupported.
//
// There is no kind for a function. Functions are resolved during analysis
// and none is a value that crosses a boundary.
//
// Gap: Lists, Maps, Strings, the unit kind `None`, ranges, boxes and views
// are kinds of the language that are not written here yet.

/// One scalar of a value, before any choice of width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Bool,
    Int,
    Real,
}

/// The form of one axis of an Array. It is part of the Array's kind, so two
/// Arrays whose axes differ in form are different kinds. A kind always states
/// the form of every axis; a written type that states only a rank means every
/// axis is closed.
///
/// Gap: how the stored part of an axis that is open at an end is described,
/// given that only a finite part can be stored, is not written yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AxisKind {
    /// Bounded at both ends, written `0..100`.
    Closed,
    /// Bounded above only, written `..100`.
    OpenBelow,
    /// Bounded below only, written `0..`.
    OpenAbove,
    /// Unbounded, written `..`.
    Open,
    /// Wraps around, written `..0..100`.
    Periodic,
}

/// A kind of fact about a value that is only known at run time. A unit
/// states each one, as a fixed shape value or by naming it in its table.
///
/// A value's kind fixes its structure at compile time, including the form of
/// each Array axis. What compile time cannot fix is where an axis starts, how
/// many elements there are, and whether an Optional value is present.
///
/// Presence is a shape value like a length or a domain, but it counts
/// nothing, and an absent value is not an empty one. An absent value owns no
/// storage, and its lengths and domains do not apply: they are neither read
/// nor used to decide how much storage to provide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeField {
    /// Whether an Optional value is present.
    Presence,
    /// The number of elements in a List.
    Length,
    /// The domain of an Array: an origin and an extent for each axis.
    Domain { rank: usize },
}

/// A reference to one kind: its place in a table of kinds.
///
/// Within one checked table, two references name the same kind exactly when
/// they are equal, as far as these contracts represent kinds. It does not yet
/// represent a product's algebra, so that guarantee does not reach products
/// that differ only in algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KindRef(pub u32);

/// One kind, stated in terms of references to the kinds it is built from.
///
/// A fiber is what one site of an Array holds: plain data of fixed width,
/// with no references and nothing that is only known at run time. The fibers
/// are the scalars, `Rational`, `Complex`, `Color`, `Vector`, the rotation
/// groups, `Mat`, `Form`, `Metric`, `Product` and `Sum`. An Array, an
/// Optional, a Struct and a Tuple are kinds but not fibers.
///
/// A Product, a Sum and a Struct are identified by their names, their order
/// and what each holds; a Tuple by its positions. The name a kind was
/// declared under is not part of it. A Struct and a Product with the same
/// names and members are different kinds, and so are a Tuple and a Vector.
///
/// Gap: a product's declared algebra is also part of its identity and is not
/// represented here. Two products that differ only in algebra are therefore
/// treated as one kind, which is wrong.
///
/// Gap: the dual of a vector space is a distinct fiber and is not named
/// here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Definition {
    Bool,
    Int,
    Real,
    /// An exact fraction: a numerator and a positive denominator.
    Rational,
    /// A complex number: `re` and `im`.
    Complex,
    /// A color: `r`, `g`, `b` and `a`.
    Color,
    /// An element of a vector space over a field: a stated number of
    /// components, at least one, each a `Real`, a `Rational` or a `Complex`.
    /// An integer vector is not one, since the Ints are not a field.
    Vector {
        element: KindRef,
        count: u32,
    },
    /// A rotation of the plane.
    SO2,
    /// A rotation of space.
    SO3,
    /// A matrix with a stated number of rows and columns.
    Mat {
        rows: u32,
        columns: u32,
    },
    /// An antisymmetric form of a stated degree on a stated dimension.
    Form {
        degree: u32,
        dimension: u32,
    },
    /// A symmetric metric on a stated dimension.
    Metric {
        dimension: u32,
    },
    /// Named fiber components in order.
    Product(Vec<(String, KindRef)>),
    /// Exactly one of an ordered roster of at least two distinctly named
    /// variants, each with a fiber payload or none.
    Sum(Vec<(String, Option<KindRef>)>),
    /// One fiber at every site of a domain. The kind states how many axes the
    /// domain has and the form of each. Where each axis starts and how far it
    /// extends is not part of the kind.
    Array {
        element: KindRef,
        axes: Vec<AxisKind>,
    },
    /// A value of the inner kind, or absence. Optional kinds do not nest:
    /// "absent or absent or a value" is the same as "absent or a value".
    Optional(KindRef),
    /// Named members in order, each of any kind.
    Struct(Vec<(String, KindRef)>),
    /// Positions in order, each of any kind.
    Tuple(Vec<KindRef>),
}

/// Why a table of kinds is not well formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindDefect {
    /// A definition refers to a kind that is not stated before it. Kinds are
    /// built only from earlier kinds, so no kind contains itself.
    NotYetStated { at: KindRef, names: KindRef },
    /// A fiber is required and the kind named is not one: the element of an
    /// Array or a Vector, a component of a Product, the payload of a Sum.
    NotAFiber { at: KindRef, names: KindRef },
    /// The element of a Vector is a fiber that is not a field. A vector space
    /// is over `Real`, `Rational` or `Complex`.
    NotAField { at: KindRef, names: KindRef },
    /// A Vector has no components, or a Sum has fewer than two variants or
    /// repeats a variant name.
    Malformed { at: KindRef },
    /// An Optional directly holds an Optional.
    NestedOptional { at: KindRef },
    /// Two definitions state the same kind. A kind is stated once.
    StatedTwice { first: KindRef, again: KindRef },
}

/// A table of kinds. Each definition refers only to definitions before it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub definitions: Vec<Definition>,
}

// ===========================================================================
// Layouts: how a value sits in storage
// ===========================================================================
//
// How a value sits in storage: its layout.
//
// A kind says what a value is. A layout says where its scalars are kept. One
// kind can have many layouts: an Array of a product may keep whole products
// side by side in one plane, keep each component in a plane of its own, or
// mix the two.
//
// A layout follows the structure of its kind and is stated in the same way:
// once, in a table, in terms of references to the layouts it is built from.
// A layout used at many places in a value appears once.
//
// A layout does not name storage. It numbers the planes it uses from zero
// and places its parts among them, so the same layout serves every value
// laid out that way. A port supplies the actual storage, one slot for each
// plane.
//
// A plane is a run of equal-sized elements. Within one element, a scalar
// sits at a byte offset. A layout says, for each of its planes, which bytes
// of an element it uses.

/// A machine scalar: how one scalar of a value is stored.
///
/// Gap: a scalar here says how a value is stored. It does not say at what
/// width arithmetic on it is done. Int arithmetic wraps at the Int's width
/// and Real arithmetic depends on precision, so that is not a layout choice
/// and nothing here grants the freedom to change it.
///
/// An Int is stored as `I32` or `I64` and a Real as `F32` or `F64`. A Bool
/// is stored as `I32` holding 0 for false or 1 for true; no other value is a
/// Bool. The check on a description cannot see the stored values, so keeping
/// to 0 and 1 is the duty of whoever writes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    I32,
    I64,
    F32,
    F64,
}

/// One plane of a layout, by its number: a run of equal elements. A layout
/// numbers the planes it uses from zero, and whoever holds a value laid out
/// that way supplies a piece of storage for each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Plane(pub u32);

/// A reference to one layout: its place in a table of layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayoutRef(pub u32);

/// One part of a group: a layout, moved along by a number of planes and a
/// number of bytes within an element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Part {
    pub layout: LayoutRef,
    /// The part's plane zero is this plane of the group.
    pub plane: Plane,
    /// The part's byte zero is this byte of an element.
    pub offset: u32,
}

/// One layout, stated in terms of references to the layouts it is built from.
///
/// Each layout uses some number of planes, numbered from zero, and has some
/// number of run-time facts, in order. Both follow from its parts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// One scalar, at the start of an element of one plane.
    Scalar(Scalar),
    /// Parts in the order the kind lists them. Parts placed in the same plane
    /// at different offsets are kept side by side in each element. Parts
    /// placed in different planes are kept apart. A group lays out a
    /// Product, a Struct or a Tuple, member by member. It lays out a
    /// `Rational` as its numerator and denominator, a `Complex` as `re` and
    /// `im`, and a `Color` as `r`, `g`, `b` and `a`. It lays out a Sum as an
    /// Int tag holding the active variant's position, then every variant's
    /// payload in declared order; the payloads of the variants that are not
    /// active hold zero.
    Group(Vec<Part>),
    /// A stated number of copies of one layout, each a fixed step on from the
    /// one before. The copies either share their planes and step by bytes,
    /// or take separate planes. A repeat lays out a Vector, and may stand in
    /// for any group whose parts are all alike.
    Repeat {
        part: LayoutRef,
        count: u32,
        plane_step: u32,
        offset_step: u32,
    },
    /// An Array: one element at every site of a domain, in the planes its
    /// element uses. Its one fact is the domain.
    Array(LayoutRef),
    /// An Optional: a value that may be absent. Its first fact is presence,
    /// followed by the facts of what it holds.
    Optional(LayoutRef),
}

/// Why a table of layouts is not well formed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutDefect {
    /// A layout refers to one that is not stated before it.
    NotYetStated { at: LayoutRef, names: LayoutRef },
    /// A layout uses more planes than the unit has storage, or a count or
    /// byte offset does not fit.
    TooLarge { at: LayoutRef },
    /// Two parts of a group use the same bytes of the same plane. Parts are
    /// compared by the whole range of bytes each spans in a plane.
    PartsOverlap {
        at: LayoutRef,
        first: u32,
        second: u32,
    },
    /// The copies of a repeat run into one another: they share planes but
    /// step by fewer bytes than one copy spans, or step by fewer planes than
    /// one copy uses.
    CopiesOverlap { at: LayoutRef },
    /// The element of an Array has run-time facts of its own. An element is
    /// a fiber, which has none.
    FactsInElement { at: LayoutRef },
    /// Two definitions state the same layout. A layout is stated once.
    StatedTwice { first: LayoutRef, again: LayoutRef },
}

/// A table of layouts. Each definition refers only to definitions before it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layouts {
    pub definitions: Vec<Layout>,
}

/// What follows from each layout in a checked table, worked out once.
#[derive(Clone, Debug)]
pub struct Measures {
    /// How many planes each layout uses.
    pub planes: Vec<u32>,
    /// How many run-time facts each layout has.
    pub facts: Vec<u64>,
    /// For each layout, the bytes of an element it spans in each of its
    /// planes, as a start and an end.
    pub spans: Vec<Vec<(u32, u32)>>,
}

// ===========================================================================
// Programs: what a source declares
// ===========================================================================
//
// The nodes of a program's dependency graph, and what a source calls them.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);

/// The identity of one derived value. Identical computations have the same
/// identity, whatever the source calls them and whether or not it names them
/// at all.
///
/// Gap: what makes two computations identical is decided where the source is
/// analyzed and is not written here. An identity holds within one analyzed
/// program; whether it survives a change to the source is not settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UpdateId(pub u32);

/// One node of a program's dependency graph: something that has a value at
/// each version.
///
/// A State and a Param are identified by name. Two with the same
/// declaration are still different, because each has its own history. A
/// derived value is identified by what it computes. Two bindings that
/// compute the same thing are one node with one value, and a value the
/// source never names is a node like any other.
///
/// A node is the same everywhere it appears: in a unit's ports, in what the
/// runtime holds and in what is demanded. Port order and type numbering are
/// never identity. A node does not identify one value: it has a value at
/// each version and in each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    /// Written by update blocks; belongs to the run.
    State(StateId),
    /// Set from outside the run; never written by an update.
    Param(ParamId),
    /// A pure function of States, Params and other derived values. Never
    /// written. A unit may take one as an input once its value has been
    /// computed.
    Derived(DerivedId),
}

/// One name the source gives a node: the node, its spelling and its kind.
/// Several names may be given to one derived value.
#[derive(Clone, Debug)]
pub struct Declared {
    pub node: Node,
    pub label: String,
    /// A reference into the program's table of kinds.
    pub kind: KindRef,
}

/// One thing a supplier can be asked to compute.
///
/// A State stands for the value it has now. Its initial value, and the value
/// an update gives it, are different computations, so each has its own form
/// of demand. A demand says nothing about what is done with the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Demand {
    /// The value a node has: a derived value, a Param, or a State as it
    /// stands.
    Value(Node),
    /// The value a State starts with. Initial values are closed: they read no
    /// State and no Param. A State that starts absent and is later given a
    /// field computed from Params gets that field from an ordinary binding,
    /// not from its initial value.
    Initial(StateId),
    /// The value one update block gives one of the States it writes, computed
    /// from the State as it stands.
    Next { update: UpdateId, state: StateId },
}

/// One update block and the States it writes.
///
/// An update is one atomic transition of a run. Every State the block writes
/// gets its next value, each computed from the States as they stand at one
/// version, and the run moves to the next version with all of them at once.
#[derive(Clone, Debug)]
pub struct Update {
    pub id: UpdateId,
    pub writes: Vec<StateId>,
}

/// Everything one analyzed source declares.
///
/// The supplier assigns the ids. The runtime treats them as opaque and uses
/// the labels only to resolve what a user asks for.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Every kind the declared nodes use, each stated once.
    pub kinds: Kinds,
    pub declared: Vec<Declared>,
    /// Update blocks, in the order the source writes them. That is not the
    /// order in which they run. Everything one block writes on one execution
    /// takes effect together.
    pub updates: Vec<Update>,
}

// ===========================================================================
// Units: the description of compiled code
// ===========================================================================
//
// The description of one piece of compiled code.

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
///
/// A defect is a mistake by whoever wrote the description, which is a code
/// generator. It is never an error in a program, and it never happens during
/// a run: a supplier meets it when it checks a description, before anything
/// reaches the runtime.
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

/// A description that these contracts cannot carry yet: the kind of a port
/// includes a fiber whose storage is not described.
///
/// This is not a defect. The kind is valid, and a correct program can have
/// it. A supplier reports this as not supported yet, not as a mistake of its
/// own. Checking stops at the first such port, so the rest of the
/// description has not been checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The port whose kind cannot be carried.
    pub port: PortRef,
}

/// Why [`Unit::check`] did not accept a description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The description is not well formed.
    Defect(Defect),
    /// The description cannot be carried yet.
    Unsupported(Unsupported),
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

// ===========================================================================
// Supply: what a compiler offers a runtime
// ===========================================================================
//
// What a compiler front half supplies to the runtime.

/// A checked unit description and the target code it describes.
pub struct Emitted {
    pub unit: Checked,
    pub code: Code,
}

/// The front half of a compiler, as the runtime sees it.
///
/// The runtime needs nothing else from a compiler: no syntax tree, no semantic
/// graph and no knowledge of how the code was generated. Two suppliers that
/// honor this contract are interchangeable to the runtime.
///
/// The runtime demands what is to be computed and the supplier compiles it.
/// Every demand is met by the same kind of computation: read values, produce
/// values. What the runtime then does with the results is its own business
/// and no part of the demand. It makes the first State from every State's
/// initial value. It commits an update by taking the new values of all the
/// States the block writes together. It reports an observation from the
/// values of the nodes it was asked for.
///
/// An input port may be a derived value. The supplier chooses, value by
/// value, between computing it inside the unit and having it computed once
/// and passed in. This is the same kind of choice as fusing or separating the
/// steps within a unit, made at the unit's edge. When a unit lists a derived
/// value as an input, the runtime must hold that value first, demanding it if
/// it does not. The one exception is an input a unit uses only as the
/// fallback of a selection: it is needed only when it would be chosen.
/// Because a derived value is identified by what it computes, a value one
/// unit produces and another takes in is the same node in both, whether or
/// not the source names it.
///
/// Every value is the value of something at one definite version. A unit is
/// compiled once and says nothing about versions: the runtime runs it for a
/// version by binding its inputs to the values of that version, and the
/// results are the values at that version. A derived value computed to feed another
/// unit is therefore computed for the version that unit reads. Version zero
/// exists before any update, since every State has an initial value, so a
/// derived value can be computed before anything else has happened.
///
/// A unit's results depend on nothing but its inputs. A value computed at
/// one version is the value at every later version at which nothing it
/// depends on has changed, where what it depends on runs through any
/// derived values down to States and Params. The runtime may keep a computed value
/// and use it again on that basis; once computed, it never has to be
/// computed again to a different result. How the runtime notices that
/// something has changed is its own business.
///
/// A demand may be made at any time, including while units the supplier
/// produced earlier are running. Producing a new unit never disturbs code
/// already running or values already computed. The runtime owes values for
/// the latest version and may skip computing them for versions in between.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    /// One unit that meets every demand, for one target.
    fn unit(&self, demands: &[Demand], target: Target) -> Result<Emitted, Self::Diagnostic>;
}

// ===========================================================================
// Sessions: what an application asks of a runtime
// ===========================================================================
//
// What an application asks of the runtime, and what the runtime reports back.
//
// The runtime owns a run and its storage. A run may complete many atomic
// updates between publications: each update produces the next State, but a
// State becomes visible to anything outside the run only when it is
// published. Compiled computations supply the transitions and the decisions
// about cadence and stopping. Publication exposes one immutable, complete
// State with its version and the Params it was computed with. A required
// observation may hold back advancement until it is taken; optional
// rendering may not. Where the repetition of updates executes is not
// specified.

/// One State of a running program. Each completed update makes the next,
/// whether or not it is published.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64);

/// Why a run ended without a result. None of these publish anything.
///
/// None is a failure of the program. Cartan is total: compiled code always
/// completes, and an error-looking result is an ordinary value. Every stop is
/// something the runtime discovers.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The runtime could not provide storage within the memory limit. It
    /// finds this out because it handles every allocation.
    Shortage,
    Cancelled,
    /// An input was missing or did not match the unit's declared port.
    Binding(Node),
    /// The unit names a memory this runtime has not set up.
    Unplaced(Memory),
    /// The code did not do what its own description says, such as never
    /// producing a shape value it determines. This is a defect in a code
    /// generator, not a run-time outcome of the program.
    Invariant,
    /// A device or its driver failed.
    Device,
}

/// One run: one execution of a program by a runtime, from its first State
/// onward, as an application drives it.
///
/// A session coordinates a supplier and the runtime. It is not another
/// evaluator, and nothing outside the runtime writes State.
///
/// Gap: this is a placeholder and its methods are provisional. Starting a run,
/// parameters, how publication is requested or offered, delivery of results
/// and stopping conditions are not written yet. `advance` states a number of
/// updates, not how they are run or which update block runs when there are
/// several. `observe` does not say which State and Params its values belong
/// to. Both report with `Stop`, which is why a run ended; a request that is
/// refused, or that cannot be compiled, need not end the run and has no
/// result type of its own yet.
pub trait Session {
    /// A retained value: one the runtime keeps after the stages that
    /// produced it have finished, with its shape values. Reading its bytes
    /// back is a separate, explicit step.
    type Value;

    /// Complete this many updates and report the version reached.
    ///
    /// A handshake is the blocking round trip in which a runtime waits for
    /// one update to finish and accepts it before it starts the next. The
    /// word has no other meaning. Asking for several updates at once is what
    /// lets a runtime avoid a handshake on every update.
    fn advance(&mut self, updates: u64) -> Result<Version, Stop>;

    fn observe(&mut self, nodes: &[Node]) -> Result<Vec<(Node, Self::Value)>, Stop>;
}

// ===========================================================================
// Pipeline: syntax
// ===========================================================================
//
// Parsing: source text to a syntax tree with locations.

/// A parsed document or expression with source locations.
///
/// Gap: this is a placeholder for the tree the syntax component already
/// publishes. The parts of it Graph relies on are not listed yet.
#[cfg(feature = "pipeline")]
pub struct SyntaxTree {
    _private: (),
}

/// Parse text. Problems are diagnostics carried in the tree, not a failure.
#[cfg(feature = "pipeline")]
pub trait Parse {
    fn parse_document(&self, source: &str) -> SyntaxTree;
}

// ===========================================================================
// Pipeline: graph
// ===========================================================================
//
// Source meaning: names, types, dependencies and the normalized computation.

/// A resolved, typed program. Documents and libraries are supplied by the
/// caller; Graph reads no files.
///
/// Gap: this is a placeholder. What Codegen and Compile may ask of an
/// analysis is not written yet.
#[cfg(feature = "pipeline")]
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
#[cfg(feature = "pipeline")]
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
/// Gap: how Graph finds candidates is not written yet.
#[cfg(feature = "pipeline")]
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Which derived value it is.
    pub value: DerivedId,
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
#[cfg(feature = "pipeline")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub backing: Vec<Backing>,
}

/// A normalized computation under one fusion plan.
///
/// The same type whether the request is the start of a run, an update or an
/// observation made later. One pipeline serves all three; the request states
/// demands and does not select a different compiler.
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
/// No indexed value takes its domain from its own contents. A domain may
/// depend on data, but only on values computed before it is filled, such as
/// a site of Ints held by an upstream Array.
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
#[cfg(feature = "pipeline")]
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
#[cfg(feature = "pipeline")]
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
#[cfg(feature = "pipeline")]
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
        demands: &[Demand],
    ) -> Result<Self::Navigator, Self::Diagnostic>;
}

// ===========================================================================
// Pipeline: code generation
// ===========================================================================
//
// Target emission: a computation becomes code and its unit description.

/// Lower one computation for one target, describing the result as one `Unit`.
///
/// Codegen chooses the physical representation: how each scalar is stored,
/// which planes a value uses and where each part sits in them. It does not choose what the computation contains or which
/// values are its inputs.
///
/// Gap: a value is bound to a port only if their layouts are identical, yet
/// nothing here tells Codegen the layout of a value that already exists, or
/// asks for a conversion. Two units can agree on a kind and still disagree on
/// its layout.
#[cfg(feature = "pipeline")]
pub trait Emit {
    type Diagnostic;

    fn emit(&self, computation: &Computation, target: Target) -> Result<Emitted, Self::Diagnostic>;
}

// ===========================================================================
// Pipeline: preparation
// ===========================================================================
//
// Immutable preparation: emitted text or bytes become loadable code.

/// Prepared code with the checked unit description passed through untouched.
///
/// It is immutable and reusable: the same prepared code can be loaded for
/// several devices of one architecture and run any number of times.
///
/// Gap: preparation accepts any supplier's output, yet it sits in the
/// pipeline layer, so a runtime built without that layer has no preparation
/// contract. And `prepare` works inside a live backend context while its
/// result is meant to be reusable across devices; whether the result is
/// tied to that context is not said.
#[cfg(feature = "pipeline")]
pub struct Prepared<C> {
    unit: Checked,
    code: C,
}

/// Turn emitted text or bytes into loadable code inside an existing backend
/// context, so a request made after launch joins the running program. It
/// accepts any supplier's output.
#[cfg(feature = "pipeline")]
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

// The checks that a description is well formed, and every other `impl`.
mod checks;

#[cfg(test)]
mod tests;
