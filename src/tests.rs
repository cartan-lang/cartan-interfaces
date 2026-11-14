//! Tests of the well-formedness checks.

mod kind {
    use crate::*;

    fn named<R: Copy>(members: &[(&str, R)]) -> Vec<(String, R)> {
        members
            .iter()
            .map(|(name, member)| (name.to_string(), *member))
            .collect()
    }

    /// The kind of a value that is one fiber.
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

        // Three fibers and five kinds are stated, however deeply they nest.
        assert_eq!((kinds.fibers.len(), kinds.kinds.len()), (3, 5));
        // Stating one again gives the same reference and adds nothing.
        assert_eq!(kinds.fiber(Fiber::Real), real);
        assert_eq!(kinds.define(Kind::Fiber(real)), time);
        assert_eq!((kinds.fibers.len(), kinds.kinds.len()), (3, 5));

        // Each level has two Arrays of two scalars and one Real.
        assert_eq!(
            kinds.scalar_counts()[pair.0 as usize],
            Some(2 * 2 * (2 + 2 + 1))
        );
        // Every Array member keeps its own domain: eight in all.
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
        // A million million scalars, counted without writing any out.
        assert_eq!(kinds.scalar_counts()[kind.0 as usize], Some(1 << 40));
        // Comparing it with a copy in another table visits each pair once.
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
        // An Array of Structs cannot be written at all: the element of an
        // Array is a fiber, and a Struct is not one.
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
        // A sum needs two variants, with different names.
        let malformed = Err(KindDefect::Malformed { at: first });
        assert_eq!(fibers(vec![Fiber::Sum(vec![variant("Only")])]), malformed);
        let twice = vec![variant("Same"), variant("Same")];
        assert_eq!(fibers(vec![Fiber::Sum(twice)]), malformed);
        // A Vector has at least one component, over any of the fields.
        let vector = |over, count| fibers(vec![Fiber::Vector { over, count }]);
        assert_eq!(vector(Field::Rational, 3), Ok(()));
        assert_eq!(vector(Field::Complex, 3), Ok(()));
        assert_eq!(vector(Field::Real, 0), malformed);
        // A fiber is stated once.
        let defect = KindDefect::FiberStatedTwice { first, again };
        assert_eq!(fibers(vec![Fiber::Real, Fiber::Real]), Err(defect));

        let (first, again) = (KindRef(0), KindRef(1));
        let real = Kind::Fiber(FiberRef(0));
        // A kind names a fiber that is in the table.
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
        // A kind is stated once.
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
        // The answer carries through anything built from it.
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

    /// Kinds for an Array of a product holding a Real and an Int.
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

        // Whole products side by side: both scalars in one 12-byte element.
        let whole = layouts.define(Layout::Group(vec![part(real, 0, 0), part(int, 0, 8)]));
        let whole = layouts.define(Layout::Array(whole));
        // Each component in a plane of its own.
        let apart = layouts.define(Layout::Group(vec![part(real, 0, 0), part(int, 1, 0)]));
        let apart = layouts.define(Layout::Array(apart));

        let measures = layouts.measure(8).unwrap();
        assert!(layouts.fits(whole, &kinds, field));
        assert!(layouts.fits(apart, &kinds, field));
        assert_eq!(measures.spans[whole.0 as usize], [(0, 12)]);
        assert_eq!(measures.spans[apart.0 as usize], [(0, 8), (0, 4)]);
        // An Array has one fact, its domain, whichever way it is laid out.
        assert_eq!(measures.facts[whole.0 as usize], 1);
        assert_eq!(measures.facts[apart.0 as usize], 1);
    }

    #[test]
    fn a_layout_must_follow_its_kind() {
        let (kinds, field) = particles();
        let mut layouts = Layouts::default();
        let real = layouts.define(Layout::Scalar(Scalar::F64));
        let long = layouts.define(Layout::Scalar(Scalar::I64));
        // The Real stored as an integer.
        let wrong = layouts.define(Layout::Group(vec![part(long, 0, 0), part(long, 0, 8)]));
        let wrong = layouts.define(Layout::Array(wrong));
        // One component missing.
        let short = layouts.define(Layout::Group(vec![part(real, 0, 0)]));
        let short = layouts.define(Layout::Array(short));
        layouts.measure(8).unwrap();
        assert!(!layouts.fits(wrong, &kinds, field));
        assert!(!layouts.fits(short, &kinds, field));
        // The element's layout without the Array around it.
        assert!(!layouts.fits(real, &kinds, field));
    }

    #[test]
    fn parts_and_copies_may_not_share_bytes() {
        let table = |definitions: Vec<Layout>| Layouts { definitions }.measure(8).err();
        let real = Layout::Scalar(Scalar::F64);
        let at = LayoutRef(1);

        // A second scalar placed four bytes into the first.
        let clash = Layout::Group(vec![part(LayoutRef(0), 0, 0), part(LayoutRef(0), 0, 4)]);
        let overlap = LayoutDefect::PartsOverlap {
            at,
            first: 0,
            second: 1,
        };
        assert_eq!(table(vec![real.clone(), clash]), Some(overlap));

        // Copies of an eight-byte scalar stepping by four bytes.
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

        // Two thousand scalars in each element, in four definitions.
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
        // Four Arrays, so four domains.
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
        // The same without the tag.
        let untagged = layouts.define(Layout::Group(vec![part(scalar, 0, 4), part(pair, 0, 8)]));

        let measures = layouts.measure(1).unwrap();
        assert!(layouts.fits(sum, &kinds, shape));
        assert!(!layouts.fits(untagged, &kinds, shape));
        assert_eq!(measures.spans[sum.0 as usize], [(0, 16)]);
    }
}

mod unit {
    use crate::*;

    /// The defect a check found, if it found one.
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
        assert_eq!(unit.check().defect(), Some(defect));
    }

    #[test]
    fn a_stage_consumes_only_what_was_produced_before_it() {
        let late = Defect::ShapeNotYetKnown {
            stage: 0,
            field: ShapeRef(0),
        };
        // Nothing produces the domain.
        let never = one_result(at(0), at(0), vec![stage(&[0], &[0], &[])]);
        assert_eq!(never.check().defect(), Some(late));
        // A stage cannot produce the capacity of storage it is writing.
        let same = one_result(at(0), at(0), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(same.check().defect(), Some(late));
        // One stage determines the domain and the next fills.
        let stages = vec![stage(&[], &[], &[0]), stage(&[0], &[0], &[])];
        assert!(one_result(at(0), at(0), stages).check().is_ok());

        // No Array takes its shape from its own contents. A stage that fills a
        // result uses its domain, so it cannot also determine it, whatever
        // storage it was given.
        let own = one_result(at(0), cells(100), vec![stage(&[0], &[0], &[0])]);
        assert_eq!(own.check().defect(), Some(late));
        // Nor may it leave the domain out of what it uses.
        let unused = Defect::GoverningShapeUnused {
            stage: 0,
            field: ShapeRef(0),
        };
        let blind = one_result(at(0), cells(100), vec![stage(&[0], &[], &[0])]);
        assert_eq!(blind.check().defect(), Some(unused));
    }

    #[test]
    fn several_ports_may_share_a_shape_value_but_it_has_one_producer() {
        // An input Array and a Struct of two Arrays, all with the domain in
        // shape field 0. The input produces it; nothing restates it.
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

        // A stage may not determine a shape value an input brings.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(0)));
        let again = vec![stage(&[], &[], &[0]), stage(&[1, 2], &[0], &[])];
        assert_eq!(shared(again).check().defect(), twice);
        // Nor may two stages determine one shape value.
        let twice = Some(Defect::ShapeProducedTwice(ShapeRef(1)));
        let both = vec![stage(&[], &[], &[1]), stage(&[], &[], &[1])];
        assert_eq!(shared(both).check().defect(), twice);
    }

    /// A unit whose only result is an Array of a Real and an Int, laid out by
    /// the given parts in the given storage.
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

        // The Int placed on top of the Real.
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
        // A Real stored as an integer.
        let mismatch = Some(Defect::KindMismatch(RESULT));
        let wrong = vec![(Scalar::I64, 0, 0), (Scalar::I32, 0, 8)];
        assert_eq!(particles(wrong, vec![out(16)]).check().defect(), mismatch);
        // Two planes laid out and one slot given.
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

        // Presence counts nothing, so it cannot state how much storage to provide.
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
            unit.check().defect(),
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
        assert_eq!(pair(early, (1, 0)).check().defect(), Some(late));

        // Two Arrays with different domains cannot be kept side by side in the
        // elements of one plane.
        let shared = Defect::PlaneSharedAcrossParts {
            port: RESULT,
            plane: Plane(0),
        };
        assert_eq!(pair(Vec::new(), (0, 8)).check().defect(), Some(shared));
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
        // 2^40 scalars against a layout of one: refused at once.
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

        // The State when present, the initial value otherwise. Nothing is
        // computed, so the unit has no stages and no storage of its own.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(1)));
        assert!(unit.check().is_ok());

        // The State as it stands, present or not.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, None));
        assert!(unit.check().is_ok());

        // The two must be kept the same way, so the result has one layout.
        let unit = current(Scalar::F32, |array, _| choose(array, 0, Some(1)));
        assert_eq!(unit.check().defect(), mismatch);

        // The first of two must be an Optional: the initial value never is.
        let unit = current(Scalar::F64, |array, _| choose(array, 1, Some(0)));
        assert_eq!(unit.check().defect(), mismatch);

        // The result is of the fallback's kind, not the Optional's.
        let unit = current(Scalar::F64, |_, optional| choose(optional, 0, Some(1)));
        assert_eq!(unit.check().defect(), mismatch);

        // A selection must name inputs the unit has.
        let unit = current(Scalar::F64, |array, _| choose(array, 0, Some(7)));
        assert_eq!(unit.check().defect(), Some(Defect::UnknownInput(selected)));
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
