//! Tests for structural validation.

mod kind {
    use crate::*;

    fn named<R: Copy>(members: &[(&str, R)]) -> Vec<(String, R)> {
        members
            .iter()
            .map(|(name, member)| (name.to_string(), *member))
            .collect()
    }

    /// The kind corresponding to a single fiber.
    fn whole(kinds: &mut Kinds, fiber: Fiber) -> KindRef {
        let fiber = kinds.fiber(fiber);
        kinds.define(Kind::Fiber(fiber))
    }

    #[test]
    fn nested_structs_with_array_members_state_each_kind_once() {
        let mut kinds = Kinds::default();
        let real = kinds.fiber(Fiber::Real);
        let int = kinds.fiber(Fiber::Int);
        let particle = kinds.fiber(Fiber::Product(named(&[("mass", real), ("id", int)])));
        let field = kinds.define(Kind::Array {
            element: particle,
            axes: vec![AxisKind::Closed],
        });
        let time = kinds.define(Kind::Fiber(real));
        let level = named(&[("cells", field), ("ghosts", field), ("time", time)]);
        let level = kinds.define(Kind::Struct(level));
        let mesh = kinds.define(Kind::Struct(named(&[("coarse", level), ("fine", level)])));
        let pair = kinds.define(Kind::Tuple(vec![mesh, mesh]));
        assert_eq!(kinds.check(), Ok(()));

        // Nesting preserves three fiber definitions and five kind
        // definitions.
        assert_eq!((kinds.fibers.len(), kinds.kinds.len()), (3, 5));
        // Redefinition returns the existing reference without adding an
        // entry.
        assert_eq!(kinds.fiber(Fiber::Real), real);
        assert_eq!(kinds.define(Kind::Fiber(real)), time);
        assert_eq!((kinds.fibers.len(), kinds.kinds.len()), (3, 5));

        // Each level has two Arrays of two scalars and one Real.
        assert_eq!(
            kinds.scalar_counts()[pair.0 as usize],
            Some(2 * 2 * (2 + 2 + 1))
        );
        // Each Array member has its own domain: eight domains in total.
        let domains = vec![ShapeField::Domain { rank: 1 }; 8];
        assert_eq!(kinds.shape_fields(pair), domains);
    }

    #[test]
    fn sharing_keeps_a_doubling_kind_small() {
        // Forty Tuples, each holding the one before it twice.
        let mut kinds = Kinds::default();
        let mut kind = whole(&mut kinds, Fiber::Real);
        for _ in 0..40 {
            kind = kinds.define(Kind::Tuple(vec![kind, kind]));
        }
        assert_eq!(kinds.check(), Ok(()));
        assert_eq!(kinds.kinds.len(), 41);
        // Count 2^40 scalars without expanding their representation.
        assert_eq!(kinds.scalar_counts()[kind.0 as usize], Some(1 << 40));
        // Cross-table comparison visits each shared pair once.
        assert!(kinds.same(kind, &kinds.clone(), kind));
    }

    #[test]
    fn the_same_kind_in_two_tables_is_recognized_whatever_its_place() {
        let mut ours = Kinds::default();
        let real = whole(&mut ours, Fiber::Real);
        let ours_state = ours.define(Kind::Optional(real));
        let mut theirs = Kinds::default();
        whole(&mut theirs, Fiber::Int);
        let real = whole(&mut theirs, Fiber::Real);
        let theirs_state = theirs.define(Kind::Optional(real));
        assert_ne!(ours_state, theirs_state);
        assert!(ours.same(ours_state, &theirs, theirs_state));
        assert!(!ours.same(ours_state, &theirs, real));
    }

    #[test]
    fn a_struct_is_not_a_product_and_a_tuple_is_not_a_vector() {
        // The types exclude Arrays of Structs: an Array element must be a
        // fiber.
        let mut kinds = Kinds::default();
        let real = kinds.fiber(Fiber::Real);
        let member = kinds.define(Kind::Fiber(real));
        let product = Fiber::Product(named(&[("x", real), ("y", real)]));
        let product = whole(&mut kinds, product);
        let structure = kinds.define(Kind::Struct(named(&[("x", member), ("y", member)])));
        let vector = Fiber::Vector {
            over: Field::Real,
            count: 2,
        };
        let vector = whole(&mut kinds, vector);
        let tuple = kinds.define(Kind::Tuple(vec![member, member]));
        assert_eq!(kinds.check(), Ok(()));
        assert!(!kinds.same(product, &kinds, structure));
        assert!(!kinds.same(vector, &kinds, tuple));
    }

    #[test]
    fn a_table_is_refused_for_each_broken_rule() {
        let fibers = |fibers: Vec<Fiber>| {
            let kinds = Vec::new();
            Kinds { fibers, kinds }.check()
        };
        let kinds = |kinds: Vec<Kind>| {
            let fibers = vec![Fiber::Real];
            Kinds { fibers, kinds }.check()
        };
        let variant = |name: &str| (name.to_string(), None);
        let (first, again) = (FiberRef(0), FiberRef(1));

        // A fiber may not refer to itself or to a later fiber.
        let itself = fibers(vec![Fiber::Product(named(&[("x", first)]))]);
        let defect = KindDefect::FiberNotYetStated {
            at: first,
            names: first,
        };
        assert_eq!(itself, Err(defect));
        // A Sum requires at least two distinctly named variants.
        let malformed = Err(KindDefect::Malformed { at: first });
        assert_eq!(fibers(vec![Fiber::Sum(vec![variant("Only")])]), malformed);
        let twice = vec![variant("Same"), variant("Same")];
        assert_eq!(fibers(vec![Fiber::Sum(twice)]), malformed);
        // A Vector requires at least one component over a supported field.
        let vector = |over, count| fibers(vec![Fiber::Vector { over, count }]);
        assert_eq!(vector(Field::Rational, 3), Ok(()));
        assert_eq!(vector(Field::Complex, 3), Ok(()));
        assert_eq!(vector(Field::Real, 0), malformed);
        // Fiber definitions are unique.
        let defect = KindDefect::FiberStatedTwice { first, again };
        assert_eq!(fibers(vec![Fiber::Real, Fiber::Real]), Err(defect));

        let (first, again) = (KindRef(0), KindRef(1));
        let real = Kind::Fiber(FiberRef(0));
        // Kind references must identify an existing fiber.
        let names = FiberRef(5);
        let defect = KindDefect::UnknownFiber { at: first, names };
        assert_eq!(kinds(vec![Kind::Fiber(names)]), Err(defect));
        // A kind may not refer to itself or to a later kind.
        let itself = kinds(vec![real.clone(), Kind::Optional(again)]);
        let defect = KindDefect::NotYetStated {
            at: again,
            names: again,
        };
        assert_eq!(itself, Err(defect));
        // An Optional never directly holds an Optional.
        let optional = Kind::Optional(first);
        let nested = kinds(vec![real.clone(), optional, Kind::Optional(again)]);
        assert_eq!(nested, Err(KindDefect::NestedOptional { at: KindRef(2) }));
        // Kind definitions are unique.
        let defect = KindDefect::StatedTwice { first, again };
        assert_eq!(kinds(vec![real.clone(), real]), Err(defect));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_answers_nothing() {
        let mut kinds = Kinds::default();
        let at = kinds.fiber(Fiber::Vector {
            over: Field::Real,
            count: 3,
        });
        let facing = kinds.fiber(Fiber::SO3);
        let pose = kinds.fiber(Fiber::Product(named(&[("at", at), ("facing", facing)])));
        let [at, facing, pose] = [at, facing, pose].map(|fiber| kinds.define(Kind::Fiber(fiber)));
        assert_eq!(kinds.check(), Ok(()));
        // Unsupported storage propagates through composite kinds.
        let counts = kinds.scalar_counts();
        assert_eq!(counts[at.0 as usize], Some(3));
        assert_eq!(counts[facing.0 as usize], None);
        assert_eq!(counts[pose.0 as usize], None);
    }
}

mod layout {
    use crate::*;

    fn part(layout: LayoutRef, plane: u32, offset: u32) -> Part {
        Part {
            layout,
            plane: Plane(plane),
            offset,
        }
    }

    /// Kinds for an Array whose Product element contains a Real and an Int.
    fn particles() -> (Kinds, KindRef) {
        let mut kinds = Kinds::default();
        let real = kinds.fiber(Fiber::Real);
        let int = kinds.fiber(Fiber::Int);
        let members = vec![("mass".to_string(), real), ("id".to_string(), int)];
        let particle = kinds.fiber(Fiber::Product(members));
        let field = kinds.define(Kind::Array {
            element: particle,
            axes: vec![AxisKind::Closed],
        });
        (kinds, field)
    }

    #[test]
    fn one_kind_may_be_laid_out_whole_or_by_component() {
        let (kinds, field) = particles();
        let mut layouts = Layouts::default();
        let real = layouts.define(Layout::Scalar(Scalar::F64));
        let int = layouts.define(Layout::Scalar(Scalar::I32));

        // Both Product components occupy one 12-byte element.
        let whole = layouts.define(Layout::Group(vec![part(real, 0, 0), part(int, 0, 8)]));
        let whole = layouts.define(Layout::Array(whole));
        // Each component occupies a separate plane.
        let apart = layouts.define(Layout::Group(vec![part(real, 0, 0), part(int, 1, 0)]));
        let apart = layouts.define(Layout::Array(apart));

        let measures = layouts.measure(8).unwrap();
        assert!(layouts.fits(whole, &kinds, field));
        assert!(layouts.fits(apart, &kinds, field));
        assert_eq!(measures.spans[whole.0 as usize], [(0, 12)]);
        assert_eq!(measures.spans[apart.0 as usize], [(0, 8), (0, 4)]);
        // Either layout gives the Array one shape field: its domain.
        assert_eq!(measures.facts[whole.0 as usize], 1);
        assert_eq!(measures.facts[apart.0 as usize], 1);
    }

    #[test]
    fn a_layout_must_follow_its_kind() {
        let (kinds, field) = particles();
        let mut layouts = Layouts::default();
        let real = layouts.define(Layout::Scalar(Scalar::F64));
        let long = layouts.define(Layout::Scalar(Scalar::I64));
        // Invalid integer representation of a Real.
        let wrong = layouts.define(Layout::Group(vec![part(long, 0, 0), part(long, 0, 8)]));
        let wrong = layouts.define(Layout::Array(wrong));
        // A missing Product component.
        let short = layouts.define(Layout::Group(vec![part(real, 0, 0)]));
        let short = layouts.define(Layout::Array(short));
        layouts.measure(8).unwrap();
        assert!(!layouts.fits(wrong, &kinds, field));
        assert!(!layouts.fits(short, &kinds, field));
        // An element layout without the required enclosing Array.
        assert!(!layouts.fits(real, &kinds, field));
    }

    #[test]
    fn parts_and_copies_may_not_share_bytes() {
        let table = |definitions: Vec<Layout>| Layouts { definitions }.measure(8).err();
        let real = Layout::Scalar(Scalar::F64);
        let at = LayoutRef(1);

        // The second scalar overlaps the first at a four-byte offset.
        let clash = Layout::Group(vec![part(LayoutRef(0), 0, 0), part(LayoutRef(0), 0, 4)]);
        let overlap = LayoutDefect::PartsOverlap {
            at,
            first: 0,
            second: 1,
        };
        assert_eq!(table(vec![real.clone(), clash]), Some(overlap));

        // Eight-byte scalars overlap at a four-byte stride.
        let crowded = Layout::Repeat {
            part: LayoutRef(0),
            count: 3,
            plane_step: 0,
            offset_step: 4,
        };
        assert!(matches!(
            table(vec![real.clone(), crowded]),
            Some(LayoutDefect::CopiesOverlap { .. })
        ));

        // The element of an Array may not itself hold an Array.
        let nested = vec![real, Layout::Array(LayoutRef(0)), Layout::Array(at)];
        let at = LayoutRef(2);
        assert_eq!(table(nested), Some(LayoutDefect::FactsInElement { at }));
    }

    #[test]
    fn a_wide_vector_is_laid_out_once_not_once_per_component() {
        let mut kinds = Kinds::default();
        let wide = kinds.fiber(Fiber::Vector {
            over: Field::Complex,
            count: 1000,
        });
        let field = kinds.define(Kind::Array {
            element: wide,
            axes: vec![AxisKind::Closed],
        });
        assert_eq!(kinds.check(), Ok(()));

        let mut layouts = Layouts::default();
        let scalar = layouts.define(Layout::Scalar(Scalar::F32));
        let pair = layouts.define(Layout::Repeat {
            part: scalar,
            count: 2,
            plane_step: 0,
            offset_step: 4,
        });
        let row = layouts.define(Layout::Repeat {
            part: pair,
            count: 1000,
            plane_step: 0,
            offset_step: 8,
        });
        let array = layouts.define(Layout::Array(row));

        // Four definitions represent two thousand scalars per element.
        assert_eq!(layouts.definitions.len(), 4);
        let measures = layouts.measure(1).unwrap();
        assert_eq!(measures.spans[array.0 as usize], [(0, 8000)]);
        assert!(layouts.fits(array, &kinds, field));
    }

    #[test]
    fn a_struct_keeps_each_array_member_in_its_own_planes_with_its_own_domain() {
        let (mut kinds, field) = particles();
        let real = kinds.fiber(Fiber::Real);
        let real = kinds.define(Kind::Fiber(real));
        let level = vec![
            ("cells".to_string(), field),
            ("ghosts".to_string(), field),
            ("time".to_string(), real),
        ];
        let level = kinds.define(Kind::Struct(level));
        let mesh = vec![("coarse".to_string(), level), ("fine".to_string(), level)];
        let mesh = kinds.define(Kind::Struct(mesh));

        let mut layouts = Layouts::default();
        let scalar = layouts.define(Layout::Scalar(Scalar::F64));
        let id = layouts.define(Layout::Scalar(Scalar::I32));
        let particle = layouts.define(Layout::Group(vec![part(scalar, 0, 0), part(id, 0, 8)]));
        let array = layouts.define(Layout::Array(particle));
        let level_layout = vec![part(array, 0, 0), part(array, 1, 0), part(scalar, 2, 0)];
        let level_layout = layouts.define(Layout::Group(level_layout));
        let mesh_layout = vec![part(level_layout, 0, 0), part(level_layout, 3, 0)];
        let mesh_layout = layouts.define(Layout::Group(mesh_layout));

        // Six definitions lay out both levels of both meshes.
        assert_eq!(layouts.definitions.len(), 6);
        let measures = layouts.measure(6).unwrap();
        assert!(layouts.fits(mesh_layout, &kinds, mesh));
        assert_eq!(measures.planes[mesh_layout.0 as usize], 6);
        // Four Arrays require four domain fields.
        assert_eq!(measures.facts[mesh_layout.0 as usize], 4);
    }

    #[test]
    fn a_sum_is_laid_out_as_a_tag_then_every_payload() {
        let mut kinds = Kinds::default();
        let real = kinds.fiber(Fiber::Real);
        let complex = kinds.fiber(Fiber::Complex);
        let shape = kinds.fiber(Fiber::Sum(vec![
            ("Point".to_string(), None),
            ("Segment".to_string(), Some(real)),
            ("Phase".to_string(), Some(complex)),
        ]));
        let shape = kinds.define(Kind::Fiber(shape));

        let mut layouts = Layouts::default();
        let tag = layouts.define(Layout::Scalar(Scalar::I32));
        let scalar = layouts.define(Layout::Scalar(Scalar::F32));
        let pair = layouts.define(Layout::Repeat {
            part: scalar,
            count: 2,
            plane_step: 0,
            offset_step: 4,
        });
        // The tag, then the Real of `Segment`, then the two Reals of `Phase`.
        let parts = vec![part(tag, 0, 0), part(scalar, 0, 4), part(pair, 0, 8)];
        let sum = layouts.define(Layout::Group(parts));
        // The same payload layout with its required tag omitted.
        let untagged = layouts.define(Layout::Group(vec![part(scalar, 0, 4), part(pair, 0, 8)]));

        let measures = layouts.measure(1).unwrap();
        assert!(layouts.fits(sum, &kinds, shape));
        assert!(!layouts.fits(untagged, &kinds, shape));
        assert_eq!(measures.spans[sum.0 as usize], [(0, 16)]);
    }
}

mod unit {
    use crate::*;

    /// Extract a validation defect when present.
    trait Outcome {
        fn defect(self) -> Option<Defect>;
    }

    impl Outcome for Result<Checked, Refusal> {
        fn defect(self) -> Option<Defect> {
            match self {
                Err(Refusal::Defect(defect)) => Some(defect),
                _ => None,
            }
        }
    }

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

    /// Construct a stage with the specified writes, shape dependencies and
    /// shape outputs.
    fn stage(writes: &[u32], uses: &[u32], determines: &[u32]) -> Stage {
        let fields = |fields: &[u32]| fields.iter().map(|field| ShapeRef(*field)).collect();
        Stage {
            entry: String::new(),
            reads: Vec::new(),
            writes: writes.iter().map(|slot| Slot(*slot)).collect(),
            uses: fields(uses),
            determines: fields(determines),
            extent: None,
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

    /// Unit tables with construction helpers.
    #[derive(Default)]
    struct Tables {
        kinds: Kinds,
        layouts: Layouts,
    }

    impl Tables {
        /// A rank-1 Array of one scalar fiber, stored as the given scalar.
        fn array(&mut self, element: Fiber, stored: Scalar) -> (KindRef, LayoutRef) {
            let element = self.kinds.fiber(element);
            let kind = self.kinds.define(Kind::Array {
                element,
                axes: vec![AxisKind::Closed],
            });
            let scalar = self.layouts.define(Layout::Scalar(stored));
            (kind, self.layouts.define(Layout::Array(scalar)))
        }

        fn reals(&mut self) -> (KindRef, LayoutRef) {
            self.array(Fiber::Real, Scalar::F64)
        }

        /// Construct an input or output State port.
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

    /// A unit with one Array of Reals as its sole result, stored in slot 0.
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
        assert_eq!(unit.check().defect(), Some(defect));
    }

    #[test]
    fn a_stage_consumes_only_what_was_produced_before_it() {
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        // The domain has no producer.
        let never = one_result(at(0), at(0), vec![stage(&[0], &[0], &[])]);
        assert_eq!(never.check().defect(), Some(late));
        // A stage cannot determine its own writable storage capacity.
        let same = one_result(at(0), at(0), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(same.check().defect(), Some(late));
        // Domain production precedes filling in a later stage.
        let stages = vec![stage(&[], &[], &[0]), stage(&[0], &[0], &[])];
        assert!(one_result(at(0), at(0), stages).check().is_ok());

        // A result-writing stage must consume its domain and cannot also
        // determine it, regardless of the storage provided.
        let own = one_result(at(0), cells(100), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(own.check().defect(), Some(late));
        // The stage must declare its domain dependency.
        let unused = Defect::GoverningShapeUnused {
            stage: 0,
            field: ShapeRef(0),
        };
        let blind = one_result(at(0), cells(100), vec![stage(&[0], &[], &[0])]);
        assert_eq!(blind.check().defect(), Some(unused));
    }

    #[test]
    fn a_stage_is_launched_over_a_domain_the_runtime_already_holds() {
        let over = |stage: Stage, extent: ShapeEntry| Stage {
            extent: Some(extent),
            ..stage
        };
        // A stage determines the domain; a later stage is launched over it.
        let measure = stage(&[], &[], &[0]);
        let fill = over(stage(&[0], &[0], &[]), at(0));
        // The stage's code finds its extent in the frame only if it uses it.
        let blind = over(stage(&[], &[], &[]), at(0));
        let unit = one_result(at(0), at(0), vec![stage(&[], &[], &[0]), blind]);
        assert_eq!(unit.check().defect(), Some(Defect::ExtentUnused(1)));
        let checked = one_result(at(0), at(0), vec![measure.clone(), fill.clone()])
            .check()
            .expect("a well-formed unit");
        assert!(checked
            .consumers(ShapeRef(0))
            .contains(&Consumer::Extent(1)));
        // A stage cannot be launched over a domain it determines itself.
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        let own = over(stage(&[], &[], &[0]), at(0));
        let unit = one_result(at(0), at(0), vec![own, stage(&[0], &[0], &[])]);
        assert_eq!(unit.check().defect(), Some(late));
        // A fixed extent needs no producer, and a presence is not an extent.
        let fixed = over(stage(&[0], &[0], &[]), cells(4));
        let unit = one_result(at(0), at(0), vec![measure.clone(), fixed]);
        assert!(unit.check().is_ok());
        let absent = ShapeEntry::Fixed(ShapeValue::Presence(true));
        let wrong = over(stage(&[0], &[0], &[]), absent);
        let unit = one_result(at(0), at(0), vec![measure, wrong]);
        assert_eq!(unit.check().defect(), Some(Defect::ExtentNotACount(1)));
    }

    #[test]
    fn several_ports_may_share_a_shape_value_but_it_has_one_producer() {
        // An input Array and two result Arrays share shape field 0. The input
        // is the domain's sole producer.
        let shared = |stages: Vec<Stage>| {
            let mut tables = Tables::default();
            let (array, array_layout) = tables.reals();
            let members = vec![("left".to_string(), array), ("right".to_string(), array)];
            let pair = tables.kinds.define(Kind::Struct(members));
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

        // A stage cannot redefine an input's shape value.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(0)));
        let again = vec![stage(&[], &[], &[0]), stage(&[1, 2], &[0], &[])];
        assert_eq!(shared(again).check().defect(), twice);
        // Two stages cannot produce the same shape value.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(1)));
        let both = vec![stage(&[], &[], &[1]), stage(&[], &[], &[1])];
        assert_eq!(shared(both).check().defect(), twice);
    }

    /// A unit returning an Array of Real–Int Products with the specified
    /// layout parts and storage.
    fn particles(parts: Vec<(Scalar, u32, u32)>, storage: Vec<Storage>) -> Unit {
        let mut tables = Tables::default();
        let real = tables.kinds.fiber(Fiber::Real);
        let int = tables.kinds.fiber(Fiber::Int);
        let members = vec![("mass".to_string(), real), ("id".to_string(), int)];
        let particle = tables.kinds.fiber(Fiber::Product(members));
        let kind = tables.kinds.define(Kind::Array {
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

        // The Int overlaps the Real.
        let clash = vec![(Scalar::F64, 0, 0), (Scalar::I32, 0, 4)];
        assert!(matches!(
            particles(clash, vec![out(16)]).check().defect(),
            Some(Defect::Layout(LayoutDefect::PartsOverlap { .. }))
        ));
        // Twelve bytes of fiber in an eight-byte element.
        let outside = Defect::OutsideElement {
            port: RESULT,
            plane: Plane(0),
        };
        assert_eq!(
            particles(together, vec![out(8)]).check().defect(),
            Some(outside)
        );
        // Invalid integer representation of a Real.
        let mismatch = Some(Defect::KindMismatch(RESULT));
        let wrong = vec![(Scalar::I64, 0, 0), (Scalar::I32, 0, 8)];
        assert_eq!(particles(wrong, vec![out(16)]).check().defect(), mismatch);
        // Two layout planes have only one storage slot.
        let mut short = particles(apart, vec![out(8), out(4)]);
        short.outputs[0].planes.pop();
        assert_eq!(short.check().defect(), mismatch);
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
        assert_eq!(unit.check().defect(), Some(repeated));
    }

    #[test]
    fn a_bool_is_stored_as_a_32_bit_integer() {
        let stored = |scalar| {
            let mut tables = Tables::default();
            let (kind, layout) = tables.array(Fiber::Bool, scalar);
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
        assert_eq!(stored(Scalar::I64).check().defect(), mismatch);
        assert_eq!(stored(Scalar::F32).check().defect(), mismatch);
    }

    #[test]
    fn a_kind_is_written_as_a_document_writes_it() {
        let mut kinds = Kinds::default();
        let real = kinds.fiber(Fiber::Real);
        let members = vec![("rho".to_string(), real), ("mom".to_string(), real)];
        let element = kinds.fiber(Fiber::Product(members));
        let array = kinds.define(Kind::Array {
            element,
            axes: vec![AxisKind::Closed, AxisKind::Closed],
        });
        let optional = kinds.define(Kind::Optional(array));
        let written = "Array<(rho: Real, mom: Real), 2>|None";
        assert_eq!(kinds.notation(optional), written);
    }

    #[test]
    fn a_table_may_hold_layouts_the_unit_does_not_use() {
        // One slot, and a table that also holds a three-plane layout for
        // some other unit of the same program.
        let mut unit = one_result(cells(4), cells(4), vec![stage(&[0], &[], &[])]);
        let real = unit.layouts.define(Layout::Scalar(Scalar::F64));
        let parts = (0..3).map(|plane| part(real, plane, 0)).collect();
        let wide = unit.layouts.define(Layout::Group(parts));
        unit.layouts.define(Layout::Array(wide));
        assert!(unit.check().is_ok());
    }

    #[test]
    fn layouts_are_compared_across_tables_by_structure() {
        let table = |filler: bool, offset: u32| {
            let mut layouts = Layouts::default();
            if filler {
                layouts.define(Layout::Scalar(Scalar::I32));
            }
            let real = layouts.define(Layout::Scalar(Scalar::F32));
            let parts = vec![part(real, 0, 0), part(real, 0, offset)];
            let group = layouts.define(Layout::Group(parts));
            let array = layouts.define(Layout::Array(group));
            (layouts, array)
        };
        let (mine, a) = table(false, 4);
        let (theirs, b) = table(true, 4);
        let (packed, c) = table(false, 8);
        assert_ne!(a, b);
        assert!(mine.same(a, &theirs, b));
        assert!(!mine.same(a, &packed, c));
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
        let mut kind = tables.kinds.define(Kind::Optional(array));
        if nest {
            kind = tables.kinds.define(Kind::Optional(kind));
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

        // Presence is not an element count and cannot determine capacity.
        let presence = ShapeEntry::Fixed(ShapeValue::Presence(true));
        let unit = optional(false, vec![at(0), at(1)], presence, measure_then_fill());
        assert_eq!(
            unit.check().defect(),
            Some(Defect::CapacityNotACount(Slot(0)))
        );

        // An Optional never directly holds an Optional.
        let unit = optional(true, vec![at(0), at(1)], cells(100), measure_then_fill());
        let defect = KindDefect::NestedOptional { at: KindRef(2) };
        assert_eq!(unit.check().defect(), Some(Defect::Kind(defect)));
    }

    #[test]
    fn a_result_fixed_as_absent_owes_no_shape() {
        // The absent result needs neither a domain producer nor a storage
        // write.
        let absent = ShapeEntry::Fixed(ShapeValue::Presence(false));
        let unit = optional(false, vec![absent, at(1)], cells(0), Vec::new());
        assert!(unit.check().is_ok());

        // A potentially present result requires a domain producer.
        let unit = optional(
            false,
            vec![at(0), at(1)],
            cells(0),
            vec![stage(&[], &[], &[0])],
        );
        assert_eq!(
            unit.check().defect(),
            Some(Defect::ResultShapeUnknown(ShapeRef(1)))
        );
    }

    /// A unit returning a Struct of two Real Arrays. The first has a fixed
    /// domain in plane 0. The second uses shape field 0 and the plane and
    /// offset specified by `second`.
    fn pair(stages: Vec<Stage>, second: (u32, u32)) -> Unit {
        let mut tables = Tables::default();
        let (array, array_layout) = tables.reals();
        let members = vec![("first".to_string(), array), ("second".to_string(), array)];
        let kind = tables.kinds.define(Kind::Struct(members));
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
        // Fill the first Array, derive the second's domain, then fill the
        // second.
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
        assert_eq!(pair(early, (1, 0)).check().defect(), Some(late));

        // Arrays with different domains cannot share elements of one plane.
        let shared = Defect::PlaneSharedAcrossParts {
            port: RESULT,
            plane: Plane(0),
        };
        assert_eq!(pair(Vec::new(), (0, 8)).check().defect(), Some(shared));
    }

    #[test]
    fn every_result_sharing_storage_has_its_shape_before_it_is_written() {
        // Two results share a slot. One has a fixed domain; the other's
        // domain is determined by the stage writing that slot.
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
        assert_eq!(unit.check().defect(), Some(defect));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_is_refused_as_unsupported() {
        let mut tables = Tables::default();
        let matrix = Fiber::Mat {
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
        let unsupported = Refusal::Unsupported(Unsupported { port: RESULT });
        assert_eq!(unit.check().err(), Some(unsupported));
    }

    #[test]
    fn a_deeply_shared_kind_is_checked_without_walking_every_scalar() {
        let mut tables = Tables::default();
        let real = tables.kinds.fiber(Fiber::Real);
        let mut kind = tables.kinds.define(Kind::Fiber(real));
        for _ in 0..40 {
            kind = tables.kinds.define(Kind::Tuple(vec![kind, kind]));
        }
        let layout = tables.layouts.define(Layout::Scalar(Scalar::F64));
        let port = tables.port(kind, layout, Vec::new(), &[0]);
        let unit = Unit {
            outputs: vec![port],
            storage: vec![out(8)],
            ..tables.unit()
        };
        // Reject 2^40 scalars against a one-scalar layout without expansion.
        assert_eq!(unit.check().defect(), Some(Defect::KindMismatch(RESULT)));
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

        // Emptiness and absence are distinct.
        let (empty, absent) = (domain(&[4, 0]), ShapeValue::Presence(false));
        assert!(empty.is_empty() && !empty.is_absent());
        assert!(absent.is_absent() && !absent.is_empty());
    }

    /// A selection-only unit for `current = state ?? initial`. State is an
    /// Optional Array; the fallback is an Array with the specified scalar
    /// representation.
    fn current(initial: Scalar, selection: impl Fn(KindRef, KindRef) -> Selection) -> Unit {
        let mut tables = Tables::default();
        let (array, array_layout) = tables.reals();
        let optional = tables.kinds.define(Kind::Optional(array));
        let optional_layout = tables.layouts.define(Layout::Optional(array_layout));
        let (_, initial_layout) = tables.array(Fiber::Real, initial);
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

        // Select the present State or its fallback without stages or
        // additional storage.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(1)));
        assert!(unit.check().is_ok());

        // Return the current State, including possible absence.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, None));
        assert!(unit.check().is_ok());

        // Both alternatives must share the result layout.
        let unit = current(Scalar::F32, |array, _| choose(array, 0, Some(1)));
        assert_eq!(unit.check().defect(), mismatch);

        // The first alternative must be Optional; this fallback is not.
        let unit = current(Scalar::F64, |array, _| choose(array, 1, Some(0)));
        assert_eq!(unit.check().defect(), mismatch);

        // The result has the fallback's kind rather than the first input's
        // Optional kind.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, Some(1)));
        assert_eq!(unit.check().defect(), mismatch);

        // Selections must reference existing inputs.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(7)));
        assert_eq!(unit.check().defect(), Some(Defect::UnknownInput(selected)));
    }

    #[test]
    fn capacity_may_be_the_shape_determined_or_a_bound() {
        // Read an Array from slot 0 with domain field 0. One stage determines
        // result domain field 1; another fills slot 1 using that domain. This
        // models a filter whose result is no larger than its input.
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
        // Exact-capacity allocation consumes the computed count.
        let (input, count) = (ShapeRef(0), ShapeRef(1));
        let exact = filter(at(1)).check().unwrap();
        assert_eq!(exact.producer(input), Some(Producer::Input(0)));
        assert_eq!(exact.producer(count), Some(Producer::Stage(0)));
        let stored = Consumer::Capacity(Slot(1));
        let consumers = [Consumer::Stage(1), stored, Consumer::Result(0)];
        assert_eq!(exact.consumers(count), consumers);

        // Input-sized allocation requires no computed shape value. The code
        // must ensure the result fits; validation cannot prove this.
        let bounded = filter(at(0)).check().unwrap();
        assert_eq!(
            bounded.consumers(count),
            [Consumer::Stage(1), Consumer::Result(0)]
        );
    }

    #[test]
    fn a_frame_lists_regions_then_the_shape_values_used_and_determined() {
        // The stage reads slot 3, writes slot 1, consumes presence and a
        // rank-2 domain, and produces a length.
        let fields = [
            ShapeField::Presence,
            ShapeField::Domain { rank: 2 },
            ShapeField::Length,
        ];
        let stage = Stage {
            reads: vec![Slot(3)],
            ..stage(&[1], &[0, 1], &[2])
        };
        let frame = stage.frame(&fields);
        assert_eq!(frame.slots, [Slot(3), Slot(1)]);
        // Two region words, one for presence, four for the domain, one for
        // the length.
        let shapes = [(ShapeRef(0), 2), (ShapeRef(1), 3), (ShapeRef(2), 7)];
        assert_eq!(frame.shapes, shapes);
        assert_eq!(frame.words, 8);
    }
}
