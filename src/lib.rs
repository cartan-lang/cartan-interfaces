//! Draft contracts between Cartan components.
//!
//! This file contains all type and trait declarations in reading order. Each
//! type's documentation defines its terminology. The crate has no
//! dependencies and represents descriptions rather than implementations.
//! Accessors and well-formedness checks are in `checks.rs`; tests are in
//! `tests.rs`.
//!
//! The contracts define kinds, layouts, programs, units and supply, with
//! the versions and stops of a run: the boundary between a compiler and a
//! runtime. What lies inside either is not stated here, so any compiler can
//! implement [`Supplier`] for the same runtime. What an application asks of
//! a runtime is not stated either, while both live in one repository.
//!
//! A code generator describes compiled code as a [`Unit`]: its inputs,
//! results, storage requirements and stages. The runtime binds inputs,
//! provides storage, launches stages and publishes results. Compiled code
//! neither allocates memory nor publishes results.
//!
//! All contracts are drafts. Paragraphs beginning `Gap:` identify unspecified
//! parts; those beginning `Decide:` identify open design choices.

// ===========================================================================
// Kinds: logical value structure
// ===========================================================================
//
// Kinds identify the language values that cross component boundaries.
// Analysis assigns kinds to expressions; this section defines their shared
// representation.
//
// A kind is logical: `Int` specifies neither a bit width nor a storage
// location. Ports specify those representation choices.
//
// A fiber is fixed-width plain data and is the only permitted Array element.
// Kinds consist of fibers or structures built from fibers and other kinds.
// The type definitions exclude invalid combinations such as Arrays of
// Structs.
//
// Fibers and kinds are defined once in separate tables and referenced by
// index. Nested Structs and fibers shared by several Arrays therefore need
// only one definition and one derivation of their properties.
//
// The fiber definitions are not exhaustive and do not require every component
// to support every fiber. Storage representations remain unspecified where
// their definition would be costly; components may report those fibers as
// unsupported.
//
// Functions are resolved during analysis and are not values that cross these
// boundaries.
//
// Gap: Lists, Maps, the unit kind `None`, ranges, boxes and views
// are not yet represented.

/// A scalar category independent of representation width.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    Bool,
    Int,
    Real,
}

/// The form of an Array axis, as part of the Array's kind. Arrays with
/// different axis forms have different kinds. Every axis has an explicit
/// form here. A type written in a document with only a rank leaves each form
/// to be inferred from the values assigned.
///
/// Gap: the representation of a finite stored portion of an axis with an open
/// end is unspecified.
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

/// A category of runtime shape information. A unit supplies each shape field
/// as a fixed value or a reference into its shape table.
///
/// A kind fixes structural properties, including Array axis forms, at compile
/// time. Shape fields describe axis origins and extents, List lengths, String
/// encoded lengths and Optional presence.
///
/// Presence is a shape field but does not count elements. An absent value is
/// distinct from an empty value: it owns no storage, and its lengths and
/// domains are inapplicable. Those fields are neither read nor used to
/// determine storage capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeField {
    /// Whether an Optional value is present.
    Presence,
    /// The number of elements in a List.
    Length,
    /// The number of UTF-8 code units in a String, not a character count.
    EncodedLength,
    /// The domain of an Array: an origin and an extent for each axis.
    Domain { rank: usize },
}

/// The scalar field of a vector space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Real,
    Rational,
    Complex,
}

/// An index into a table of fiber definitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FiberRef(pub u32);

/// Fixed-width plain data suitable for one Array element. A fiber contains
/// neither references nor information determined only at run time.
///
/// Composite fibers refer to their constituent fibers. Product and Sum
/// identity includes member names, order and payload types, but excludes the
/// name under which the fiber was declared.
///
/// Gap: a Product's declared algebra is part of its identity but is not
/// represented. Products differing only in algebra are therefore incorrectly
/// identified as the same fiber.
///
/// Gap: the dual of a vector space is a distinct fiber but is not
/// represented.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Fiber {
    Bool,
    Int,
    Real,
    /// An exact fraction with a numerator and a positive denominator.
    Rational,
    /// A complex number: `re` and `im`.
    Complex,
    /// A color: `r`, `g`, `b` and `a`.
    Color,
    /// An element of a vector space with at least one component over the
    /// specified field. Integer vectors are excluded because the integers do
    /// not form a field.
    Vector {
        over: Field,
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
    /// Named components in order.
    Product(Vec<(String, FiberRef)>),
    /// Exactly one of at least two ordered, distinctly named variants, each
    /// with an optional payload.
    Sum(Vec<(String, Option<FiberRef>)>),
}

/// An index into a table of kind definitions.
///
/// Within a checked table, references are equal exactly when they denote the
/// same represented kind. This guarantee excludes distinctions not yet
/// represented, such as Products differing only in algebra.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KindRef(pub u32);

/// The logical structure of a value, expressed through references to its
/// constituent fibers and kinds.
///
/// An Array contains a fiber at each site. Optionals, Structs and Tuples may
/// contain values of any kind. The types enforce the restriction that Array
/// elements are fibers.
///
/// Struct identity includes member names, order and kinds; Tuple identity
/// follows its positions. Declaration names are excluded. A Struct and a
/// Product remain distinct even with identical names and members, as do a
/// Tuple and a Vector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Decoded Unicode text, distinct from a fiber or a collection of scalars.
    String,
    /// A single fiber value.
    Fiber(FiberRef),
    /// One fiber at each site of a domain. The kind specifies the number and
    /// form of its axes; origins and extents belong to the value's shape.
    Array {
        element: FiberRef,
        axes: Vec<AxisKind>,
    },
    /// A value of the inner kind or absence. Directly nested Optionals are
    /// excluded: repeated absence has the same meaning as a single absence.
    Optional(KindRef),
    /// Named members in order, each of any kind.
    Struct(Vec<(String, KindRef)>),
    /// Positions in order, each of any kind.
    Tuple(Vec<KindRef>),
}

/// A violation of the fiber or kind table's structural requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindDefect {
    /// A fiber references itself or a later definition. References must point
    /// to earlier fibers, excluding recursive definitions.
    FiberNotYetStated { at: FiberRef, names: FiberRef },
    /// A Vector has no components, or a Sum has fewer than two variants or
    /// repeats a variant name.
    Malformed { at: FiberRef },
    /// Duplicate fiber definitions.
    FiberStatedTwice { first: FiberRef, again: FiberRef },
    /// A kind references an undefined fiber.
    UnknownFiber { at: KindRef, names: FiberRef },
    /// A kind references itself or a later definition. References must point
    /// to earlier kinds, excluding recursive definitions.
    NotYetStated { at: KindRef, names: KindRef },
    /// An Optional directly holds an Optional.
    NestedOptional { at: KindRef },
    /// Duplicate kind definitions.
    StatedTwice { first: KindRef, again: KindRef },
}

/// Unique fiber and kind definitions. References to fibers within a fiber, or
/// to kinds within a kind, must point to earlier entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kinds {
    pub fibers: Vec<Fiber>,
    pub kinds: Vec<Kind>,
}

// ===========================================================================
// Layouts: physical value representation
// ===========================================================================
//
// A layout specifies where a value's scalars are stored. A kind may admit
// several layouts: an Array of Products may store each Product in one plane,
// distribute its components across planes, or combine these arrangements.
//
// Layouts follow kind structure and are defined once in a table, with
// references to constituent layouts. Shared layouts do not require repeated
// definitions.
//
// A layout identifies planes by zero-based indices, independently of any
// allocation. A port supplies one storage slot per plane.
//
// A plane is a sequence of equally sized elements. Scalars occupy byte
// offsets within an element; the layout specifies the byte range used in each
// plane.

/// The machine representation of a scalar.
///
/// Gap: storage width does not specify arithmetic width. Int arithmetic wraps
/// at its arithmetic width, and Real arithmetic depends on precision. These
/// are semantic properties, not layout choices; this contract does not permit
/// changing them.
///
/// An Int uses `I32` or `I64`; a Real uses `F32` or `F64`. A Bool uses `I32`
/// with 0 for false and 1 for true. Description checks cannot inspect stored
/// values, so writers must enforce this Boolean representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scalar {
    I32,
    I64,
    F32,
    F64,
}

/// A zero-based plane index within a layout. Each plane contains equally
/// sized elements and requires storage supplied by the value's holder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Plane(pub u32);

/// An index into a table of layout definitions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayoutRef(pub u32);

/// A constituent layout positioned within a group by plane and byte offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Part {
    pub layout: LayoutRef,
    /// The group's plane corresponding to this part's plane zero.
    pub plane: Plane,
    /// The byte offset of this part within an element.
    pub offset: u32,
}

/// A layout defined through references to constituent layouts. Its plane
/// count and ordered shape fields are derived from those constituents.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Layout {
    /// UTF-8 text in one plane of one-byte code units, without a terminator.
    /// Its sole shape field is the encoded length. Published content is valid
    /// UTF-8; this does not prescribe the spelling of a String literal.
    Utf8,
    /// One scalar, at the start of an element of one plane.
    Scalar(Scalar),
    /// Parts in kind order. Parts sharing a plane occupy different byte
    /// offsets within each element; parts in separate planes have separate
    /// storage.
    ///
    /// A group represents Product, Struct and Tuple members in order; a
    /// Rational's numerator and denominator; a Complex value's `re` and `im`;
    /// or a Color's `r`, `g`, `b` and `a`. A Sum uses an Int tag for the
    /// active variant's position, followed by all variant payloads in
    /// declaration order. Inactive payloads contain zero.
    Group(Vec<Part>),
    /// A fixed number of equally spaced copies of one layout. Copies either
    /// share planes at distinct byte offsets or occupy separate planes. This
    /// represents a Vector or any group of identical parts.
    Repeat {
        part: LayoutRef,
        count: u32,
        plane_step: u32,
        offset_step: u32,
    },
    /// An Array with one element at each domain site, using the element's
    /// planes. Its sole shape field is the domain.
    ///
    /// Sites are stored in the order of their indices with the last axis
    /// varying fastest: the element at index `(i, j)` of a domain with
    /// origins `(a, b)` and extents `(m, n)` is element `(i - a) * n +
    /// (j - b)` of each plane.
    Array(LayoutRef),
    /// An Optional whose shape fields begin with presence, followed by those
    /// of its inner value.
    Optional(LayoutRef),
}

/// A violation of a layout table's structural requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutDefect {
    /// A layout references itself or a later definition.
    NotYetStated { at: LayoutRef, names: LayoutRef },
    /// A layout exceeds the available plane count, or a count or byte offset
    /// exceeds its representation.
    TooLarge { at: LayoutRef },
    /// Two group parts overlap within a plane. Overlap is checked over each
    /// part's full byte span.
    PartsOverlap {
        at: LayoutRef,
        first: u32,
        second: u32,
    },
    /// Repeated copies overlap: their byte stride is smaller than one copy's
    /// span in shared planes, or their plane stride is smaller than one
    /// copy's plane count.
    CopiesOverlap { at: LayoutRef },
    /// An Array element has shape fields. Array elements must be fibers,
    /// which have no shape fields.
    FactsInElement { at: LayoutRef },
    /// Duplicate layout definitions.
    StatedTwice { first: LayoutRef, again: LayoutRef },
}

/// Layout definitions whose references point only to earlier entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layouts {
    pub definitions: Vec<Layout>,
}

/// Properties derived once for each layout in a checked table.
#[derive(Clone, Debug)]
pub struct Measures {
    /// The plane count of each layout.
    pub planes: Vec<u32>,
    /// The shape-field count of each layout.
    pub facts: Vec<u64>,
    /// The start and end of each layout's byte span within an element of each
    /// plane.
    pub spans: Vec<Vec<(u32, u32)>>,
}

// ===========================================================================
// Programs: source declarations
// ===========================================================================
//
// Program dependency nodes and their source names.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateId(pub u32);

/// A Param is fixed over a run: its value is given, or defaults, when the run
/// begins and does not change until another run begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);

/// The identity of a derived value. Identical computations share an identity
/// regardless of source names, including unnamed computations.
///
/// Gap: analysis determines computational identity, but its criteria are
/// unspecified here. Identity is valid within one analyzed program and remains
/// stable through successful expression registration. Independently analyzed
/// source revisions need not preserve it; queries must be resolved again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DerivedId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UpdateId(pub u32);

/// A program dependency node with a value at each version.
///
/// States and Params are identified by name. Distinct declarations retain
/// separate identities and histories even when their definitions are
/// identical. Derived values are identified by computation: bindings with
/// identical computations share one node. Unnamed derived values have the
/// same status as named ones.
///
/// Node identity is consistent across unit ports, retained values and
/// demands. Port order and type-table indices do not establish identity. A
/// node identifies a computation or declaration, with a value at each version
/// in each run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Node {
    /// State owned by the run and written by update blocks.
    State(StateId),
    /// A parameter set externally and never written by an update.
    Param(ParamId),
    /// A pure function of States, Params and other derived values. It is
    /// never assigned; once computed, its value may be bound as a unit input.
    Derived(DerivedId),
}

/// A source name, node identity and kind. Several names may refer to the same
/// derived value.
#[derive(Clone, Debug)]
pub struct Declared {
    pub node: Node,
    pub label: String,
    /// A reference into the program's table of kinds.
    pub kind: KindRef,
    /// The type the author wrote on this declaration, as written, when there
    /// is one. It is how the declaration is shown to the author. A name in
    /// it is an alias: it has no part in the kind, and a kind is never shown
    /// by a name the author did not write there.
    pub written: Option<String>,
}

/// A computation requested from a supplier.
///
/// A State's current value, initial value and next value are distinct
/// computations with distinct demand forms. A demand does not specify how the
/// runtime uses the result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Demand {
    /// The current value of a derived node, Param or State.
    Value(Node),
    /// A State's initial value. Initial values are closed: they read neither
    /// States nor Params. If a State begins absent and later receives a field
    /// computed from Params, an ordinary binding supplies that field.
    Initial(StateId),
    /// A Param's default value when none is supplied at launch. Defaults are
    /// closed under the same restriction as initial values.
    ///
    /// A value given at launch is stored with the layout of this demand's
    /// result, so every unit reads a Param one way.
    Default(ParamId),
    /// The next value assigned to one State by an update block, computed from
    /// the current State.
    Next { update: UpdateId, state: StateId },
}

/// An update block and the States it writes.
///
/// An update is an atomic transition. All next values are computed from one
/// version of the States, then take effect together as the run advances to
/// the next version.
///
/// Decide: which update a run performs next when a program has several. A
/// runtime that takes them in declared order, one for each version, is
/// consistent with what is stated.
#[derive(Clone, Debug)]
pub struct Update {
    pub id: UpdateId,
    pub writes: Vec<StateId>,
}

/// The declarations of an analyzed source. The supplier assigns identifiers;
/// the runtime treats them as opaque and uses labels to resolve user
/// requests.
#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Unique kind definitions used by declarations and registered expressions.
    pub kinds: Kinds,
    pub declared: Vec<Declared>,
    /// Update blocks in source order, which does not prescribe execution
    /// order. All writes from one execution of a block take effect
    /// atomically.
    pub updates: Vec<Update>,
}

// ===========================================================================
// Units: compiled code descriptions
// ===========================================================================

/// One axis of an Array domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Axis {
    pub origin: i64,
    pub extent: u64,
}

/// A value's runtime shape information: List length, Array domain, String
/// encoded length or Optional presence.
///
/// Shape describes element counts, locations or presence, independently of
/// byte size. Element size is fixed at compile time. A unit never specifies
/// an allocation's byte count; the runtime derives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeValue {
    /// Optional presence, distinct from an element count.
    Presence(bool),
    Length(u64),
    /// UTF-8 code units, independent of allocation capacity and character count.
    EncodedLength(u64),
    Domain(Vec<Axis>),
}

/// An index into a unit's shape-value table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapeRef(pub u32);

/// A fixed shape value or a reference into the unit's shape table.
///
/// Shape values are separate from a value's storage planes and remain
/// associated with any retained value. An absent value has a presence field
/// but no storage.
///
/// A shape value must be produced before use. Unlike storage, which stages
/// may write sequentially, a named shape value is immutable after production
/// and has one producer and any number of consumers. Input shape values
/// accompany the bound input; each remaining shape value is determined by one
/// stage. Consumers include stages, storage capacities and results. Several
/// ports may reference the same shape value to express equal shape.
///
/// The runtime consumes a shape value when needed to provide storage, verify
/// a specialization, select an input or answer an observation. It need not
/// read shape values consumed only by compiled code. Before a runtime
/// operation uses a shape value, the value must be valid for the current
/// inputs. If unavailable, it must be obtained before that operation
/// proceeds; this dependency alone does not delay unrelated work.
///
/// An unproduced shape value is distinct from a presence value indicating
/// absence. Shape fields inside an absent Optional are inapplicable and are
/// neither produced nor consumed. For example, computing a length when
/// present and zero otherwise consumes presence in both cases, but consumes
/// length only when present. Storage reserved for an absent result remains
/// unused. Validation checks declared dependencies, independently of which
/// shape values apply during a particular run.
///
/// On an input port, `Fixed` denotes specialization. The runtime must verify
/// the bound value's shape before any stage executes. Extent changes require
/// recompilation only when the code specializes on those extents. Inputs
/// sharing a shape reference must agree; distinct shapes are never treated as
/// equal.
///
/// Gap: physical representation, memory placement, transfer to consumers and
/// notification of inapplicable shape values are unspecified.
///
/// A result whose presence is fixed as absent states a fixed empty value
/// for each shape entry inside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeEntry {
    Fixed(ShapeValue),
    Runtime(ShapeRef),
}

/// The memory containing a storage region.
///
/// Each slot specifies its memory, allowing a unit to combine small host
/// values with buffers on one or more devices.
///
/// Decide: device indices are local to a unit and mapped to physical devices
/// by the runtime. Whether a description may identify a physical device
/// directly remains unresolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Memory {
    /// Ordinary memory of the host process.
    Host,
    /// The linear memory of the Wasm instance that runs the unit.
    Linear,
    /// The memory of one compute device, numbered within the unit.
    Device(u32),
}

/// An index into a unit's storage table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(pub u32);

/// A slot's required element capacity: the product of the counts represented
/// by these lengths and domains. An empty product denotes one element.
/// Presence is not a count and cannot contribute to capacity.
///
/// Compiled writes must fit the declared capacity. Description checks cannot
/// establish this obligation, and the runtime is not required to read shape
/// values to verify it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capacity(pub Vec<ShapeEntry>);

/// A slot's storage requirements: memory, fixed element size and element
/// capacity.
///
/// The description declares storage requirements; compiled code does not
/// request allocations. The runtime derives allocation sizes and supplies a
/// [`Region`] before the first stage that writes the slot. This operation
/// consumes exactly the shape values named by the capacity.
#[derive(Clone, Debug)]
pub struct Need {
    pub memory: Memory,
    /// Element size in bytes.
    pub element_bytes: u32,
    /// Required alignment of the storage start, in bytes.
    pub align: u32,
    /// Capacity dependencies must be produced before the first stage that
    /// writes the slot.
    pub capacity: Capacity,
}

/// Storage assigned to a slot at run time, represented by an allocation base
/// and a byte offset. A whole allocation has offset zero.
///
/// A region satisfies the slot's alignment and capacity requirements. Code
/// must access only its assigned regions and may assume neither that a region
/// starts at its allocation base nor that distinct slots occupy distinct
/// allocations. Regions for distinct slots do not overlap except through
/// aliases.
///
/// Gap: the target-specific representation of a base is unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region<Base> {
    pub base: Base,
    /// Bytes from the base to the start of the region.
    pub offset: u64,
}

/// A slot's role and write ownership.
#[derive(Clone, Debug)]
pub enum Storage {
    /// An input plane with declared memory, element size and capacity. The
    /// unit cannot write it.
    Input(Need),
    /// A result plane written by the unit and transferred to the runtime.
    Output(Need),
    /// A result plane sharing an unchanged input plane's storage. A whole
    /// input value returned unchanged is represented by `Selection`.
    Alias(Slot),
    /// Storage private to one execution of the unit, with no external
    /// identity.
    Scratch(Need),
}

/// A unit input identified by node, kind, layout, shape and storage.
///
/// The layout must represent the kind. The port supplies one shape value per
/// shape field in kind order and one slot per layout plane.
///
/// Binding requires identical layouts. Any layout conversion is an explicit
/// computation, never an implicit operation during binding.
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
    /// One storage slot per layout plane. Distinct planes of one value cannot
    /// share storage.
    pub planes: Vec<Slot>,
    /// What the code assumes of the bound value's shape beyond what the port
    /// brings: each entry says that one of the port's shape fields equals a
    /// shape value that a stage determines. The runtime compares the two
    /// once it holds the determined value, before any stage that reads the
    /// input's storage, and refuses the binding if they differ. Code may
    /// then read the input on the strength of that shape without testing it.
    pub assumes: Vec<Assumed>,
}

/// One shape field of an input, by its place in the port's `shape`, and the
/// stage-determined shape value the code assumes it equals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assumed {
    pub field: u32,
    pub equals: ShapeRef,
}

/// A computed result satisfying one demand. Kind, layout, shape and storage
/// are specified as for an [`InputPort`].
#[derive(Clone, Debug)]
pub struct OutputPort {
    /// The demand this result meets.
    pub demand: Demand,
    pub kind: KindRef,
    pub layout: LayoutRef,
    pub shape: Vec<ShapeEntry>,
    pub planes: Vec<Slot>,
}

/// An input returned unchanged, including its shape, layout and storage.
/// Selection performs no computation or copying.
///
/// Without `otherwise`, the result is `first`, as when returning a State's
/// current value.
///
/// With `otherwise`, `first` must be Optional. If present, its inner value is
/// returned; otherwise the fallback input is returned. The runtime selects by
/// consuming the presence of `first`. Both alternatives must have the same
/// layout, but may have different shapes; the result retains the selected
/// shape.
///
/// An input used only as a selection fallback is required only if some
/// selection chooses it. The runtime need not hold that input while every
/// relevant Optional is present, and then leaves its port unbound.
///
/// Gap: device-side selection and conditional demand for its fallback are
/// unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The demand this result meets.
    pub demand: Demand,
    /// A reference into the unit's table of kinds.
    pub kind: KindRef,
    /// The input selected unconditionally or when present.
    pub first: u32,
    /// The fallback input selected when `first` is absent.
    pub otherwise: Option<u32>,
}

/// A compiled entry point invoked in one launch. A stage is the sole
/// execution boundary in this contract: the runtime launches it, and it runs
/// to completion without calling the runtime.
///
/// Before launch, the runtime supplies consumed values and writable storage.
/// The stage produces storage contents and shape values. Runtime actions,
/// including storage provision and any associated waiting, occur between
/// stages.
///
/// Launching consecutive stages need not require additional runtime work.
/// Storage is provided only when the next stage writes a slot not yet
/// provisioned. The runtime waits for an earlier stage only when it must
/// itself read a result of that stage. Computations that fit in one launch
/// may, but need not, form one stage.
///
/// A stage consumes only values produced by bound inputs or earlier stages.
/// This rule applies equally to storage contents and shape values.
///
/// A result's shape may depend on previously computed data, but never on its
/// own contents. A stage writing part of a result consumes the shape values
/// governing that part and cannot also determine them. Shape determination
/// precedes filling in a later stage.
///
/// For fixed code, a stage's results depend only on what it consumes, all of
/// which its description declares. Storage whose previous contents affect
/// the result must appear among the stage's reads. A runtime may reuse a
/// shape value produced from unchanged consumed values without
/// waiting for it to be produced again. Other writes remain required; the
/// entire stage may be omitted only when all its outputs are already
/// available and valid. Change detection is a runtime implementation choice.
/// Treating every write as a change is sound.
///
/// The stage list specifies a valid execution order. The runtime may reorder
/// or overlap stages only if each receives the storage contents and shape
/// values that list order would provide.
///
/// A stage reports no failure. Cartan is total: compiled code completes and
/// represents every computed outcome as an ordinary value. Early termination
/// is limited to runtime-detected conditions, such as storage shortage or
/// cancellation, and occurs between stages.
///
/// A stage may perform any amount of work, including several updates,
/// provided it requires no runtime intervention. The location of repetition
/// is unspecified.
///
/// Each slot is passed as a [`Region`].
///
/// A unit supplies either compiled target code or explicit native host code.
/// Gap: mixing host and compiled stages within one unit is unspecified.
///
/// A stage is launched once, or over an extent: a domain or a length that
/// the runtime holds before the launch. A stage with an extent computes each
/// site of it independently: at one site it reads nothing the stage writes at
/// another, and so lists no slot among both its reads and its writes. A
/// target may therefore visit the sites in any order or all at once. When
/// the extent counts no sites the stage is not launched. An
/// extent that is not fixed is among the stage's `uses`, so the code finds
/// it in its frame.
///
/// A runtime that submits stages without waiting keeps the contents of a
/// region until every submitted stage that reads or writes it has completed.
///
/// Slots and shape values are passed through a [`Frame`]. [`Code::Wasm`] and
/// [`Code::Metal`] state how a stage is called and how its extent is visited.
///
/// Gap: the calling convention for CUDA is unspecified.
#[derive(Clone, Debug, Default)]
pub struct Stage {
    /// The stage's entry-point name.
    pub entry: String,
    /// Storage this stage reads.
    pub reads: Vec<Slot>,
    /// Storage this stage writes.
    pub writes: Vec<Slot>,
    /// Consumed shape values. These declare dependencies without requiring a
    /// machine load or, by themselves, a runtime read.
    pub uses: Vec<ShapeRef>,
    /// Shape values this stage produces.
    pub determines: Vec<ShapeRef>,
    /// What the stage is launched over: a domain or a length. `None` is one
    /// launch with no sites, as for a stage that only determines shape values.
    /// A shape value named here is also among `uses`.
    pub extent: Option<ShapeEntry>,
}

/// A compiled computation's runtime description.
///
/// A unit defines a shared namespace for storage, shape values, kinds and
/// layouts, and maps results to demands. Execution occurs at its stages.
/// Within and between units, every consumed value must first be produced by a
/// stage or supplied through a bound input.
///
/// Results depend only on inputs. A unit supplies values at requested sites
/// of indexed values. Inputs and results reside in memory, but may share
/// existing storage; see `Storage::Alias`.
///
/// A value's domain is distinct from the sites requested for one execution
/// and from those currently covered by storage. A port's domain always
/// denotes the value's domain.
///
/// Gap: requested sites and storage coverage smaller than the whole domain
/// are unspecified.
///
/// Scalar results, Array fills, multi-stage updates and fused kernels use the
/// same description, differing only in ports, slots and stages. Update and
/// observation demands likewise request ordinary computations over named
/// values. The runtime determines whether results become the next State;
/// computing a State's value does not itself write the State.
#[derive(Clone, Debug, Default)]
pub struct Unit {
    /// Unique kind definitions used by ports.
    pub kinds: Kinds,
    /// Unique layout definitions used by ports.
    pub layouts: Layouts,
    pub inputs: Vec<InputPort>,
    /// Results the unit computes.
    pub outputs: Vec<OutputPort>,
    /// Inputs returned unchanged as results.
    pub selected: Vec<Selection>,
    pub storage: Vec<Storage>,
    /// Unique declarations of shape values not fixed at compile time.
    pub shape_fields: Vec<ShapeField>,
    /// Stages in a valid execution order.
    pub stages: Vec<Stage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Wasm,
    Metal,
    Cuda,
}

/// A stage's launch arguments as a sequence of words: regions followed by
/// shape values. [`Stage::frame`] derives this sequence.
///
/// Words appear in read-slot order, write-slot order, consumed-shape order
/// and produced-shape order. Each slot occupies one word locating its region.
/// Presence occupies one word, either 0 or 1; length occupies one word; a
/// domain occupies two words per axis, origin followed by extent.
///
/// The runtime initializes region words and consumed shape values before
/// launch. The stage writes the shape values it determines. Words for
/// inapplicable slots or shape fields within an absent Optional are zero, and
/// code must not read through them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The slots, in frame order. Slot `i` is at word `i`.
    pub slots: Vec<Slot>,
    /// Shape references and their starting word indices, in frame order.
    pub shapes: Vec<(ShapeRef, u32)>,
    /// The frame's word count.
    pub words: u32,
}

/// An earlier text operation in a native artifact. Shared operations are stated
/// once; references never expand them into copied trees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextRef(pub u32);

/// A scalar constant with its execution width retained. Formatting these bits
/// does not evaluate an authored numerical expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextScalar {
    Bool(bool),
    Int(i32),
    /// IEEE 754 binary32 bits, including signed zero and nonfinite values.
    Real32(u32),
}

/// The decimal notation requested by a text operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextNotation {
    #[default]
    Auto,
    Fixed,
    Scientific,
    Engineering,
}

/// Validated scalar formatting options. Precision is zero through seventeen
/// digits after the decimal point, retaining trailing zeros. A Bool admits
/// only the default options and is written `true` or `false`.
///
/// Auto without precision uses exact integer text or shortest-round-trip text
/// at the Real's actual width, retaining `.0` for an integral Real without an
/// exponent. Negative zero retains its sign. Nonfinite Reals use `nan`, `inf`
/// and `-inf`, independently of options.
///
/// With precision, Auto uses Fixed for zero or magnitude in [1e-4, 1e6), and
/// Scientific otherwise. Scientific has a mantissa magnitude in [1, 10),
/// Engineering in [1, 1000) and an exponent divisible by three. Zero has a
/// zero mantissa (with its sign retained) and exponent zero. Rounding carry
/// renormalizes both. Fixed without precision uses four decimal places;
/// Scientific/Engineering without precision use three mantissa decimal places.
/// In these explicitly selected forms without precision, trailing zeros trim
/// to one decimal place. Exponents have no redundant leading zero or plus sign.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextFormat {
    pub notation: TextNotation,
    pub digits: Option<u8>,
}

/// A pure native text operation. Operands refer only to earlier operations.
/// A runtime measures concatenations with checked sums over the shared graph;
/// filling may visit a reference repeatedly without materializing intermediates.
/// Empty subtrees require no repeated traversal. No operation allocates output,
/// fetches dependencies or evaluates numerical arithmetic from a source program.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextOp {
    Literal(String),
    Concat(Vec<TextRef>),
    Format {
        value: TextScalar,
        format: TextFormat,
    },
}

/// A native text stage, aligned by index with a unit's stage list. Every stage
/// runs once with no dispatch extent, storage reads or implicit inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostStage {
    /// Measure UTF-8 code units and produce one encoded-length shape value.
    Measure { text: TextRef, output: ShapeRef },
    /// Fill exactly the encoded length in the declared Host output slot.
    /// A fixed length is permitted only for a Literal and equals its UTF-8
    /// byte length. Other operations use a measured length produced earlier.
    Fill {
        text: TextRef,
        output: Slot,
        length: ShapeEntry,
    },
}

/// A native artifact implementing a closed text computation. Several logical
/// operations and stages may belong to one unit, sharing this one text table.
/// The runtime uses its ordinary unit cache, value ownership and publication.
/// Outputs satisfy distinct Derived value demands; one computation identity
/// cannot have two result ports. Distinct demands may share unchanged result
/// storage and shape. String Params and States are outside this artifact contract.
///
/// Encoded-length overflow is Runtime-detected storage shortage ([`Stop::Shortage`]);
/// no fill or result publication follows it.
/// Counting and writing use the same formatting semantics, with checked sizes
/// and bounded output. Temporary text cannot bypass Runtime storage accounting.
/// Gap: text operands from input ports and other host services are unspecified.
#[derive(Clone, Debug, Default)]
pub struct HostCode {
    pub text: Vec<TextOp>,
    pub stages: Vec<HostStage>,
}

/// A malformed native artifact, rejected before executing any stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostDefect {
    TextReference { at: TextRef, names: TextRef },
    Format { at: TextRef },
    Inputs,
    StageCount,
    Stage { index: u32 },
    Storage { slot: Slot },
    Output { index: u32 },
}

/// Target code as text or bytes, prepared and loaded on the runtime side.
///
/// The native 32-bit mathematical functions are `exp` (the exponential),
/// `log` (the natural logarithm), and `sin` and `cos` (angles in radians).
/// Arguments and results are IEEE 754 binary32 values. These functions do
/// not trap and have no observable side effects. Their exceptional results
/// are required on every target:
///
/// | Function | Argument | Result |
/// | --- | --- | --- |
/// | All four | NaN | NaN; sign and payload unspecified |
/// | `exp` | Either signed zero | Exactly 1 |
/// | `exp` | Positive infinity | Positive infinity |
/// | `exp` | Negative infinity | Positive zero |
/// | `log` | Either signed zero | Negative infinity |
/// | `log` | Positive infinity | Positive infinity |
/// | `log` | Negative normal value or negative infinity | NaN |
/// | `sin` | Either signed zero | The same signed zero |
/// | `cos` | Either signed zero | Exactly 1 |
/// | `sin`, `cos` | Either infinity | NaN |
///
/// For a subnormal argument, `exp` and `cos` give exactly 1, and `sin`
/// preserves the argument bits. Metal's subnormal-input exception makes
/// `log` of either sign of subnormal give negative infinity. Without that
/// exception, a WebAssembly binding gives a finite logarithm for a positive
/// subnormal and NaN for a negative one.
///
/// Otherwise accuracy is the target implementation's own, with no uniform
/// error bound. Underflow and subnormal results are target dependent.
/// No additional agreement is required across runs, machines, versions or
/// targets.
///
/// Each binding is selected once for a run and must be pure: the same
/// argument bits produce the same result bits every time within that run.
/// Reusing computed values and remeasuring shapes depend on this obligation.
/// A platform whose
/// implementation cannot meet it does not conform.
pub enum Code {
    /// Native text operations implemented by the runtime. Stages correspond by
    /// index to the unit stages; entry names are descriptive only. All storage
    /// is Runtime-provided Host memory. Preparation validates the artifact
    /// against the checked unit through [`HostCode::check`].
    Host(HostCode),
    /// A binary WebAssembly module.
    ///
    /// The module imports `env.memory`, a linear memory owned and grown by
    /// the runtime. Its only other permitted imports are the functions
    /// `exp_f32`, `log_f32`, `sin_f32` and `cos_f32` from module `cartan.math`,
    /// with the mathematical meanings stated above. Each has exactly the
    /// signature `(f32) -> f32`. Any subset may appear, in any import order,
    /// including relative to memory. Preparation rejects an unknown import
    /// or a mismatching signature as invalid code, before any stage runs.
    ///
    /// The module has no start function. It may define functions it does
    /// not export. All unit slots use `Memory::Linear`.
    ///
    /// Each stage is exported under its entry-point name as a function taking
    /// one 32-bit integer and returning nothing. The argument is the byte
    /// offset of its [`Frame`] in linear memory. Frame words are 64-bit
    /// little-endian values; each region word is the region's byte offset in
    /// that memory. A region may lie at offset zero: code learns which slots
    /// apply from shape values, never by testing a region word.
    ///
    /// A stage with an extent is still one call: the function visits every
    /// site itself.
    ///
    /// Gap: a stage cannot be launched over part of its extent, as running
    /// it on several cores would need. The extent in the frame is the shape
    /// value the code also takes addresses from, so a range of sites to
    /// visit would have to be stated apart from it.
    Wasm(Vec<u8>),
    /// Metal Shading Language source, compiled by the runtime with fast
    /// math disabled, so that each operation is done as written. Its native
    /// `exp`, `log`, `sin` and `cos` follow the mathematical meanings and
    /// within-run purity obligation stated above; disabling fast math does
    /// not establish an accuracy bound for them.
    ///
    /// Each stage is a kernel function named by its entry point. Its
    /// arguments are buffers: buffer 0 holds the stage's [`Frame`] as 64-bit
    /// little-endian words, and buffer `1 + k` is the memory
    /// `Memory::Device(k)`. Each region word is the region's byte offset in
    /// its memory's buffer. All unit slots use `Memory::Device`. The frame
    /// is private to its launch until the launch completes, and the kernel
    /// may write it.
    ///
    /// A stage with an extent is dispatched over a grid with one thread for
    /// each site. A thread's position along the grid's first dimension is its
    /// site's offset from the origin on the last axis; the second dimension
    /// is the axis before that, and the third the axis before that. A thread
    /// whose position lies outside the extent does nothing, so a runtime may
    /// round the grid up. A stage with no extent is dispatched with one
    /// thread. How threads are grouped is the runtime's choice, and a kernel
    /// does not depend on it.
    ///
    /// A stage that determines shape values writes them into its frame, where
    /// the runtime reads them once the stage has completed. Scalars are those
    /// Metal has, so no 64-bit Real is stored.
    ///
    /// Gap: an extent of rank above three.
    ///
    /// Gap: a shape value determined on a device reaches a later stage on
    /// that device only through the host, which waits for it and writes it
    /// into the later frame. A determined shape value that a later frame
    /// could refer to in place would remove that wait.
    Metal(String),
    Cuda(String),
}

/// A port index within a unit's inputs, computed outputs or selections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortRef {
    Input(u32),
    Output(u32),
    Selected(u32),
}

/// A structural defect in a unit description.
///
/// Defects are errors in the code generator's description, not errors in the
/// source program or runtime outcomes. The supplier detects them when
/// checking a description before passing it to the runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    UnknownSlot(Slot),
    UnknownShapeField(ShapeRef),
    /// A consumed shape value, or one required by writable storage capacity,
    /// has no earlier producer. A stage cannot determine a shape value that
    /// governs the result it is filling.
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
    /// A stage's extent is neither a domain nor a length.
    ExtentNotACount(u32),
    /// A stage is launched over a shape value it does not list among its
    /// uses, so its code could not find the extent in its frame.
    ExtentUnused(u32),
    /// A stage with an extent lists a slot among both its reads and its
    /// writes, so one site could read what the stage writes at another.
    ReadsWhatItWrites {
        stage: u32,
        slot: Slot,
    },
    /// An input assumes a shape that is not one of its shape fields, is not
    /// of that field's sort, or is not determined by a stage.
    AssumedShapeUnknown {
        input: u32,
        equals: ShapeRef,
    },
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
    /// A port's layout, shape fields or plane count does not match its kind.
    /// This includes shape values of the wrong category.
    KindMismatch(PortRef),
    /// A selection names an input the unit does not have.
    UnknownInput(PortRef),
    /// A capacity uses presence, which is not an element count.
    CapacityNotACount(Slot),
    /// Two planes of one value are given the same storage.
    PlaneRepeated {
        port: PortRef,
        plane: Plane,
    },
    /// A plane's layout exceeds its storage element size.
    OutsideElement {
        port: PortRef,
        plane: Plane,
    },
    /// Parts governed by different shape values share one plane. A plane's
    /// elements must share a domain and presence.
    PlaneSharedAcrossParts {
        port: PortRef,
        plane: Plane,
    },
}

/// A port kind containing a fiber whose storage representation is
/// unspecified.
///
/// The kind is valid, so this is an unsupported capability rather than a
/// defect in the description. Checking stops at the first unsupported port;
/// the remaining description is not checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The port with an unsupported kind.
    pub port: PortRef,
}

/// The reason [`Unit::check`] rejected a description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The description is not well formed.
    Defect(Defect),
    /// The description uses an unsupported representation.
    Unsupported(Unsupported),
}

/// A unit description validated by [`Unit::check`]. The private fields ensure
/// that construction requires these checks.
///
/// Kind and layout tables are well formed and contain unique definitions. All
/// referenced slots and shape fields exist. Each port has a supported kind, a
/// compatible layout, one shape value per shape field and one slot per plane.
/// Selections use compatible input kinds with identical layouts.
///
/// Each value's planes use distinct storage, and each plane fits within its
/// storage element size. Capacities contain no presence fields. Stages cannot
/// write inputs or aliases; aliases reference inputs.
///
/// Shape values have at most one producer and are consumed only after that
/// producer. A stage writing part of a result declares the shape values
/// governing that part. Capacity dependencies precede the first write to
/// their storage. Every result shape field has a producer unless it belongs
/// to a part fixed as absent.
///
/// Validation alone does not establish readiness to execute. It does not
/// verify that every result is written, that storage reads follow writes,
/// that writes fit declared capacity, that alignments are usable, or that
/// code follows its description. It establishes declared ordering, not
/// runtime location or completion of shape production.
///
/// Decide: the trust required for externally supplied code is unresolved.
/// This contract does not confine code to its declared storage.
#[derive(Clone, Debug)]
pub struct Checked {
    unit: Unit,
    governs: Vec<Vec<ShapeRef>>,
}

/// The unique producer of a shape value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Producer {
    /// The input supplying this shape value. If several inputs share the
    /// reference, this identifies the first.
    Input(u32),
    /// This stage determines it.
    Stage(u32),
}

/// A declared consumer of a shape value. A shape value may have any number of
/// consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consumer {
    /// This stage uses it.
    Stage(u32),
    /// Storage whose capacity requires the runtime to consume this shape
    /// value.
    Capacity(Slot),
    /// This computed result has it.
    Result(u32),
    /// A selection whose choice depends on this shape value.
    Selection(u32),
    /// This stage is launched over it.
    Extent(u32),
    /// This input's shape is checked against it at binding.
    Input(u32),
}

// ===========================================================================
// Supply: compiler services for the runtime
// ===========================================================================

/// A checked unit description and the target code it describes.
pub struct Emitted {
    pub unit: Checked,
    pub code: Code,
}

/// A registered expression in the supplier's accepted program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expression {
    /// The ordinary value demand for this expression.
    pub node: Node,
    /// A reference into the current [`Program::kinds`].
    pub kind: KindRef,
}

/// Why expression registration did not extend the accepted program.
#[derive(Debug)]
pub enum ExpressionError<D> {
    /// This supplier does not provide expression registration.
    Unsupported,
    /// The expression was refused, with its retained source diagnostic.
    Refused(D),
}

/// The compiler interface consumed by the runtime.
///
/// The runtime requires no syntax tree, semantic graph or knowledge of code
/// generation. Suppliers satisfying this contract are interchangeable.
///
/// The runtime requests computations through demands; the supplier returns
/// code that reads and produces values. Result use is independent of the
/// demand: initial values establish the first State, update results replace
/// the written States atomically, and requested node values supply
/// observations.
///
/// For each derived value, the supplier chooses between computation inside a
/// unit and a precomputed input. This extends the choice between fused and
/// separate computations across the unit boundary. The runtime must obtain
/// derived inputs before use, requesting them when necessary. An input used
/// only as a selection fallback is required only when selected. Computation
/// identity links a derived result in one unit to the same node in another,
/// independently of source naming.
///
/// Values belong to definite versions. A compiled unit is
/// version-independent; the runtime binds inputs for the required version and
/// obtains results for that version. Derived inputs must therefore be
/// computed for the version their consumer reads. Initial State values
/// establish version zero before any update, allowing derived values to be
/// computed at that version.
///
/// A computed value remains valid at later versions while all its
/// dependencies are unchanged, including dependencies through derived values
/// down to States and Params. The runtime may retain and reuse it on that
/// basis. For unchanged dependencies, recomputation cannot produce a
/// different result. Change detection is a runtime implementation choice.
///
/// Demands may arrive while previously supplied units execute. Producing a
/// new unit must preserve running code and existing values. The runtime must
/// supply values for the latest version but may omit intermediate versions.
pub trait Supplier {
    type Diagnostic;

    fn program(&self) -> &Program;

    /// Supply one unit satisfying all demands. The target governs compiled
    /// work; explicitly admitted native host operations use [`Code::Host`].
    /// This choice precedes execution, never follows an execution failure.
    fn unit(&self, demands: &[Demand], target: Target) -> Result<Emitted, Self::Diagnostic>;

    /// Resolve one expression in the accepted root source scope, including its
    /// resolved imports, and register its ordinary value demand.
    ///
    /// Registration leaves authored declarations, updates and their meanings
    /// unchanged. It preserves every existing node's identity and meaning,
    /// including unnamed nodes. New kind definitions may be appended without
    /// changing existing kind references. Previously supplied units remain
    /// compatible with retained values and subsequent units for the same run
    /// and target. An expression denoting an existing computation returns the
    /// same node under the program's computation identity rules.
    ///
    /// Success makes the returned node available through [`Supplier::unit`].
    /// Registration does not execute the expression or select a target;
    /// target-specific refusals remain ordinary unit-supply failures. Failure
    /// leaves the accepted program and its previously available demands intact.
    /// Repeated registrations extend this same program, not independent copies.
    /// An independently analyzed source revision is a new identity scope.
    ///
    /// Diagnostics retain the exact submitted expression as a distinct source,
    /// source-relative byte spans and any referenced accepted source snapshots.
    /// They can be described after the input text and supplier are dropped.
    /// The same source retention applies to later unit-supply diagnostics for
    /// the registered expression.
    fn register_expression(
        &mut self,
        _text: &str,
    ) -> Result<Expression, ExpressionError<Self::Diagnostic>> {
        Err(ExpressionError::Unsupported)
    }

    /// A diagnostic as structured data, for an application's one formatter.
    /// A diagnostic is never identified or parsed by its text. It needs no
    /// supplier, so a document that was refused can still be described.
    fn describe(diagnostic: &Self::Diagnostic) -> Report;

    /// The inspection commands this supplier offers.
    fn inspections(&self) -> Vec<Inspection> {
        Vec::new()
    }

    /// Run an inspection, returning `None` if the command is unavailable.
    /// A view that depends on the target, such as a unit's code, shows what
    /// would be supplied for `target`, the one the session runs on.
    fn inspect(
        &self,
        component: &str,
        view: &str,
        arguments: &[String],
        target: Target,
    ) -> Option<Result<Report, Self::Diagnostic>> {
        let _ = (component, view, arguments, target);
        None
    }
}

/// A supplier-provided development inspection command.
///
/// An interactive runtime session lists and invokes these commands by name
/// without depending on the supplier's internal components. Inspections are
/// read-only and are called only by interactive sessions; they are not
/// service interfaces between components.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    /// The component the command belongs to, such as `compiler` or `syntax`.
    pub component: String,
    /// The inspection view, such as `unit` or `tree`.
    pub view: String,
    /// An argument usage line, empty for commands without arguments.
    pub arguments: String,
    /// A one-line description of the view.
    pub summary: String,
}

/// Structured inspection output. Presentation belongs to the caller;
/// inspections do not format display text.
#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    Text(String),
    Int(i64),
    Real(f64),
    Bool(bool),
    List(Vec<Report>),
    /// Named entries in order.
    Record(Vec<(String, Report)>),
}

// ===========================================================================
// Runs: versions and stops
// ===========================================================================
//
// The runtime owns each run and its storage. Several atomic updates may
// complete between publications. Each update produces the next State; only
// publication makes a State externally visible.
//
// Compiled computations supply transitions and cadence and stopping
// decisions. Publication exposes a complete immutable State with its version
// and the Params used to compute it. Required observations may delay
// advancement; optional rendering may not. The location of the update loop is
// unspecified.

/// A State version within a run. Every completed update advances the version,
/// independently of publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64);

/// A runtime-detected reason for ending a run without a result or
/// publication.
///
/// These conditions are distinct from program results. Cartan is total:
/// compiled code completes, and outcomes representing errors remain ordinary
/// values.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The runtime could not provide storage within its memory limit.
    /// Allocation is entirely runtime-owned.
    Shortage,
    Cancelled,
    /// An input was missing or did not match the unit's declared port.
    Binding(Node),
    /// The unit references memory not configured by this runtime.
    Unplaced(Memory),
    /// Compiled code violated its description, for example by failing to
    /// produce a declared shape value. This is a code generator defect, not a
    /// program outcome.
    Invariant,
    /// A device or its driver failed.
    Device,
}

// Well-formedness checks and all other implementations.
mod checks;

#[cfg(test)]
mod tests;
