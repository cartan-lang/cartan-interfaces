//! Draft contracts between Cartan components.
//!
//! This file contains all type and trait declarations in reading order. Each
//! type's documentation defines its terminology. The crate has no
//! dependencies and represents descriptions rather than implementations.
//! Accessors and well-formedness checks are in `checks.rs`; tests are in
//! `tests.rs`.
//!
//! The contracts define kinds, layouts, programs, units, supply and
//! sessions: the compiler–runtime and application–runtime boundaries. What
//! lies inside a compiler or inside a runtime is not stated here, so any
//! compiler can implement [`Supplier`] for the same runtime.
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
// Gap: Lists, Maps, Strings, the unit kind `None`, ranges, boxes and views
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
/// time. Shape fields describe axis origins and extents, List lengths and
/// Optional presence.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);

/// The identity of a derived value. Identical computations share an identity
/// regardless of source names, including unnamed computations.
///
/// Gap: analysis determines computational identity, but its criteria are
/// unspecified here. Identity is valid within one analyzed program; stability
/// across source changes is unresolved.
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
    /// Decide: the layout of a value given at launch. The proposal is the
    /// layout of this demand's result, so every unit reads a Param one way.
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
    /// Unique kind definitions used by declared nodes.
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

/// A value's runtime shape information: List length, Array domain or Optional
/// presence.
///
/// Shape describes element counts, locations or presence, independently of
/// byte size. Element size is fixed at compile time. A unit never specifies
/// an allocation's byte count; the runtime derives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShapeValue {
    /// Optional presence, distinct from an element count.
    Presence(bool),
    Length(u64),
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
/// Decide: what a result whose presence is fixed as absent states for the
/// shape entries inside it. A fixed empty value for each is accepted.
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
/// Decide: binding requires identical layouts. Any layout conversion would be
/// an explicit computation, never an implicit operation during binding.
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
/// Gap: stage placement is unspecified. Code targets one backend, so a stage
/// whose inputs are all host-available cannot be placed on the host.
///
/// Gap: the relationship between consumed shape values and dispatch extent is
/// unspecified.
///
/// The WebAssembly calling convention passes slots and shape values through a
/// [`Frame`], as specified by [`Code::Wasm`].
///
/// Gap: stage calling conventions for other targets are unspecified.
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

/// Target code as text or bytes, prepared and loaded on the runtime side.
pub enum Code {
    /// A binary WebAssembly module.
    ///
    /// The module imports only `env.memory`, a linear memory owned and grown
    /// by the runtime, and has no start function. It therefore cannot call
    /// external functions. It may define functions it does not export. All
    /// unit slots use `Memory::Linear`.
    ///
    /// Each stage is exported under its entry-point name as a function taking
    /// one 32-bit integer and returning nothing. The argument is the byte
    /// offset of its [`Frame`] in linear memory. Frame words are 64-bit
    /// little-endian values; each region word is the region's byte offset in
    /// that memory. A region may lie at offset zero: code learns which slots
    /// apply from shape values, never by testing a region word.
    Wasm(Vec<u8>),
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
}

// ===========================================================================
// Supply: compiler services for the runtime
// ===========================================================================

/// A checked unit description and the target code it describes.
pub struct Emitted {
    pub unit: Checked,
    pub code: Code,
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

    /// Compile one unit satisfying all demands for the specified target.
    fn unit(&self, demands: &[Demand], target: Target) -> Result<Emitted, Self::Diagnostic>;

    /// A diagnostic as structured data, for an application's one formatter.
    /// A diagnostic is never identified or parsed by its text. It needs no
    /// supplier, so a document that was refused can still be described.
    fn describe(diagnostic: &Self::Diagnostic) -> Report;

    /// The inspection commands this supplier offers.
    fn inspections(&self) -> Vec<Inspection> {
        Vec::new()
    }

    /// Run an inspection, returning `None` if the command is unavailable.
    fn inspect(
        &self,
        component: &str,
        view: &str,
        arguments: &[String],
    ) -> Option<Result<Report, Self::Diagnostic>> {
        let _ = (component, view, arguments);
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
// Sessions: application interaction with the runtime
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

/// An application-driven program execution from its initial State onward.
///
/// A session coordinates the supplier and runtime without evaluating the
/// language. Only the runtime writes State.
///
/// Gap: this interface is provisional. Run initialization, parameters,
/// publication requests and offers, result delivery and stopping conditions
/// are unspecified. `advance` gives an update count without defining
/// execution policy or selection among update blocks. `observe` does not
/// identify the State and Params associated with its results. Both methods
/// return `Stop`, although refusal or compilation failure need not terminate
/// a run and has no separate result type yet.
pub trait Session {
    /// A value retained by the runtime, with its shape values, after its
    /// producing stages complete. Byte readback is a separate, explicit
    /// operation.
    type Value;

    /// Complete the requested number of updates and return the resulting
    /// version.
    ///
    /// A handshake is a blocking round trip that completes and accepts one
    /// update before starting the next. Requesting several updates together
    /// allows the runtime to avoid a handshake for each update.
    fn advance(&mut self, updates: u64) -> Result<Version, Stop>;

    fn observe(&mut self, nodes: &[Node]) -> Result<Vec<(Node, Self::Value)>, Stop>;
}

// Well-formedness checks and all other implementations.
mod checks;

#[cfg(test)]
mod tests;
