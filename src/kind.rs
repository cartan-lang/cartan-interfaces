//! What a value is: its kind.
//!
//! Kinds are the language's own. This module writes down the ones that cross
//! a boundary so that every component names them the same way. The rules that
//! give an expression its kind are not here; they belong to analysis.
//!
//! A kind is logical. It says `Int`, not how many bits an Int has, and it says
//! nothing about where a value is stored. Those are choices a port states.
//!
//! A kind is stated once. Kinds live in a table, and wherever a kind is used
//! it is referred to by its place in that table. A Struct nested inside
//! another, or a fiber used by many Arrays, appears once however often it is
//! used, and its facts are worked out once.
//!
//! The fibers named here each have one definition. The list is not claimed to
//! be complete. Naming a fiber does not oblige any component to support it.
//! Where stating how a fiber is stored would be costly, this module declines
//! to: it answers "not described" and a component is free to report the
//! fiber as unsupported.
//!
//! There is no kind for a function. Functions are resolved during analysis
//! and none is a value that crosses a boundary.
//!
//! Gap: Lists, Maps, Strings, the unit kind `None`, ranges, boxes and views
//! are kinds of the language that are not written here yet.

use std::collections::{HashMap, HashSet};

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

/// A kind of fact about a value that is only known at run time. A unit's
/// shape record has one entry for each.
///
/// A value's kind fixes its structure at compile time, including the form of
/// each Array axis. What compile time cannot fix is where an axis starts, how
/// many elements there are, and whether an Optional value is present.
///
/// Presence shares the record with the sizes but is not a size. It counts
/// nothing, and an absent value is not an empty one. An absent value owns no
/// storage and its sizes do not apply: they are neither read nor used to
/// decide how much storage to provide.
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
/// they are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KindRef(pub u32);

/// One kind, stated in terms of references to the kinds it is built from.
///
/// A fiber is what one site of an Array holds: plain data of fixed width,
/// with no references and no size that is only known at run time. The fibers
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
/// Gap: being able to write a fiber here does not make it valid. The
/// language admits a vector only as a real vector space, and has a distinct
/// dual that is not named here.
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
    /// A stated number of components of one fiber, at least one.
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

impl Kinds {
    /// The reference for a kind, adding it to the table only if it is not
    /// already there.
    pub fn define(&mut self, definition: Definition) -> KindRef {
        let found = self
            .definitions
            .iter()
            .position(|known| *known == definition);
        let place = found.unwrap_or_else(|| {
            self.definitions.push(definition);
            self.definitions.len() - 1
        });
        KindRef(place as u32)
    }

    pub fn get(&self, kind: KindRef) -> Option<&Definition> {
        self.definitions.get(kind.0 as usize)
    }

    fn children(definition: &Definition) -> Vec<KindRef> {
        match definition {
            Definition::Vector { element, .. } | Definition::Array { element, .. } => {
                vec![*element]
            }
            Definition::Optional(inner) => vec![*inner],
            Definition::Product(members) | Definition::Struct(members) => {
                members.iter().map(|(_, kind)| *kind).collect()
            }
            Definition::Sum(variants) => variants.iter().filter_map(|(_, kind)| *kind).collect(),
            Definition::Tuple(positions) => positions.clone(),
            _ => Vec::new(),
        }
    }

    /// Which kinds are fibers, one answer per definition.
    pub fn fibers(&self) -> Vec<bool> {
        let not_fiber = |definition: &Definition| {
            matches!(
                definition,
                Definition::Array { .. }
                    | Definition::Optional(_)
                    | Definition::Struct(_)
                    | Definition::Tuple(_)
            )
        };
        self.definitions
            .iter()
            .map(|definition| !not_fiber(definition))
            .collect()
    }

    /// Check that the table is well formed. A checked table states each kind
    /// once, so within it equal references mean the same kind.
    pub fn check(&self) -> Result<(), KindDefect> {
        let fibers = self.fibers();
        let mut seen = HashSet::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            let at = KindRef(index as u32);
            for names in Self::children(definition) {
                if names.0 as usize >= index {
                    return Err(KindDefect::NotYetStated { at, names });
                }
            }
            let needs_fibers = matches!(
                definition,
                Definition::Vector { .. }
                    | Definition::Array { .. }
                    | Definition::Product(_)
                    | Definition::Sum(_)
            );
            if needs_fibers {
                for names in Self::children(definition) {
                    if !fibers[names.0 as usize] {
                        return Err(KindDefect::NotAFiber { at, names });
                    }
                }
            }
            let malformed = match definition {
                Definition::Vector { count, .. } => *count == 0,
                Definition::Sum(variants) => {
                    let mut names = HashSet::new();
                    variants.len() < 2 || !variants.iter().all(|(name, _)| names.insert(name))
                }
                _ => false,
            };
            if malformed {
                return Err(KindDefect::Malformed { at });
            }
            if let Definition::Optional(inner) = definition {
                if matches!(self.definitions[inner.0 as usize], Definition::Optional(_)) {
                    return Err(KindDefect::NestedOptional { at });
                }
            }
            if !seen.insert(definition) {
                let first = self
                    .definitions
                    .iter()
                    .position(|known| known == definition);
                return Err(KindDefect::StatedTwice {
                    first: KindRef(first.unwrap_or(index) as u32),
                    again: at,
                });
            }
        }
        Ok(())
    }

    /// How many scalars one element of each kind has, one answer per
    /// definition, worked out once for the whole table. The answer is nothing
    /// if the kind includes a fiber whose storage is not described, or if the
    /// count does not fit in 64 bits.
    ///
    /// The table must be checked first.
    ///
    /// Gap: the storage of `SO2`, `SO3`, `Mat`, `Form` and `Metric` is not
    /// described. A port of such a kind is refused as unsupported.
    pub fn leaf_counts(&self) -> Vec<Option<u64>> {
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.definitions.len());
        for definition in &self.definitions {
            let of = |kind: &KindRef| counts[kind.0 as usize];
            let sum = |kinds: Vec<KindRef>| {
                kinds
                    .iter()
                    .try_fold(0u64, |sum, kind| sum.checked_add(of(kind)?))
            };
            let count = match definition {
                Definition::Bool | Definition::Int | Definition::Real => Some(1),
                Definition::Rational | Definition::Complex => Some(2),
                Definition::Color => Some(4),
                Definition::Vector { element, count } => {
                    of(element).and_then(|each| each.checked_mul(u64::from(*count)))
                }
                Definition::Sum(_) => {
                    sum(Self::children(definition)).and_then(|payloads| payloads.checked_add(1))
                }
                Definition::Product(_)
                | Definition::Struct(_)
                | Definition::Tuple(_)
                | Definition::Array { .. }
                | Definition::Optional(_) => sum(Self::children(definition)),
                Definition::SO2
                | Definition::SO3
                | Definition::Mat { .. }
                | Definition::Form { .. }
                | Definition::Metric { .. } => None,
            };
            counts.push(count);
        }
        counts
    }

    /// The scalars of one element of a kind, in order, or nothing if its
    /// storage is not described. A port lists one leaf for each, in this
    /// order.
    ///
    /// A vector gives its positions first to last. A product, a struct and a
    /// tuple give their members as declared. A sum gives an Int tag holding
    /// the active variant's position, then the scalars of every variant's
    /// payload in declared order; the scalars of the variants that are not
    /// active hold zero.
    ///
    /// This writes a repeated kind out in full. Ask `leaf_counts` first.
    pub fn leaves(&self, kind: KindRef) -> Option<Vec<Primitive>> {
        let mut leaves = Vec::new();
        self.push_leaves(kind, &mut leaves)?;
        Some(leaves)
    }

    fn push_leaves(&self, kind: KindRef, leaves: &mut Vec<Primitive>) -> Option<()> {
        let definition = self.get(kind)?;
        match definition {
            Definition::Bool => leaves.push(Primitive::Bool),
            Definition::Int => leaves.push(Primitive::Int),
            Definition::Real => leaves.push(Primitive::Real),
            Definition::Rational => leaves.extend([Primitive::Int; 2]),
            Definition::Complex => leaves.extend([Primitive::Real; 2]),
            Definition::Color => leaves.extend([Primitive::Real; 4]),
            Definition::Vector { element, count } => {
                for _ in 0..*count {
                    self.push_leaves(*element, leaves)?;
                }
            }
            Definition::Sum(_) => {
                leaves.push(Primitive::Int);
                for payload in Self::children(definition) {
                    self.push_leaves(payload, leaves)?;
                }
            }
            Definition::Product(_)
            | Definition::Struct(_)
            | Definition::Tuple(_)
            | Definition::Array { .. }
            | Definition::Optional(_) => {
                for member in Self::children(definition) {
                    self.push_leaves(member, leaves)?;
                }
            }
            Definition::SO2
            | Definition::SO3
            | Definition::Mat { .. }
            | Definition::Form { .. }
            | Definition::Metric { .. } => return None,
        }
        Some(())
    }

    /// The run-time facts of a value of a kind, in order: presence first for
    /// an Optional, a domain for an Array, and the facts of each member of a
    /// Struct or Tuple in turn. A port lists one entry for each, in this
    /// order.
    pub fn shape_fields(&self, kind: KindRef) -> Vec<ShapeField> {
        let mut fields = Vec::new();
        self.push_shape_fields(kind, &mut fields);
        fields
    }

    fn push_shape_fields(&self, kind: KindRef, fields: &mut Vec<ShapeField>) {
        match self.get(kind) {
            Some(Definition::Array { axes, .. }) => {
                fields.push(ShapeField::Domain { rank: axes.len() })
            }
            Some(Definition::Optional(inner)) => {
                fields.push(ShapeField::Presence);
                self.push_shape_fields(*inner, fields);
            }
            Some(definition @ (Definition::Struct(_) | Definition::Tuple(_))) => {
                for member in Self::children(definition) {
                    self.push_shape_fields(member, fields);
                }
            }
            _ => {}
        }
    }

    /// Whether a kind in this table and a kind in another table are the same
    /// kind. Both tables must be checked. Each pair of kinds is compared
    /// once, however often it is shared.
    pub fn same(&self, kind: KindRef, other: &Kinds, theirs: KindRef) -> bool {
        self.same_with(kind, other, theirs, &mut HashMap::new())
    }

    fn same_with(
        &self,
        kind: KindRef,
        other: &Kinds,
        theirs: KindRef,
        known: &mut HashMap<(KindRef, KindRef), bool>,
    ) -> bool {
        if let Some(answer) = known.get(&(kind, theirs)) {
            return *answer;
        }
        let (Some(mine), Some(yours)) = (self.get(kind), other.get(theirs)) else {
            return false;
        };
        let (a, b) = (Self::children(mine), Self::children(yours));
        let head = |definition: &Definition| match definition {
            Definition::Vector { count, .. } => Definition::Vector {
                element: KindRef(0),
                count: *count,
            },
            Definition::Array { axes, .. } => Definition::Array {
                element: KindRef(0),
                axes: axes.clone(),
            },
            Definition::Optional(_) => Definition::Optional(KindRef(0)),
            Definition::Product(members) => Definition::Product(blank(members)),
            Definition::Struct(members) => Definition::Struct(blank(members)),
            Definition::Sum(variants) => Definition::Sum(
                variants
                    .iter()
                    .map(|(name, payload)| (name.clone(), payload.map(|_| KindRef(0))))
                    .collect(),
            ),
            Definition::Tuple(positions) => Definition::Tuple(vec![KindRef(0); positions.len()]),
            plain => plain.clone(),
        };
        let answer = head(mine) == head(yours)
            && a.len() == b.len()
            && a.iter()
                .zip(&b)
                .all(|(a, b)| self.same_with(*a, other, *b, known));
        known.insert((kind, theirs), answer);
        answer
    }
}

fn blank(members: &[(String, KindRef)]) -> Vec<(String, KindRef)> {
    members
        .iter()
        .map(|(name, _)| (name.clone(), KindRef(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(members: &[(&str, KindRef)]) -> Vec<(String, KindRef)> {
        members
            .iter()
            .map(|(name, kind)| (name.to_string(), *kind))
            .collect()
    }

    #[test]
    fn nested_structs_with_array_members_state_each_kind_once() {
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let int = kinds.define(Definition::Int);
        let particle = kinds.define(Definition::Product(named(&[("mass", real), ("id", int)])));
        let field = kinds.define(Definition::Array {
            element: particle,
            axes: vec![AxisKind::Closed],
        });
        let level = named(&[("cells", field), ("ghosts", field), ("time", real)]);
        let level = kinds.define(Definition::Struct(level));
        let mesh = kinds.define(Definition::Struct(named(&[
            ("coarse", level),
            ("fine", level),
        ])));
        let pair = kinds.define(Definition::Tuple(vec![mesh, mesh]));
        assert_eq!(kinds.check(), Ok(()));

        // Seven kinds are stated, however deeply they nest.
        assert_eq!(kinds.definitions.len(), 7);
        // Stating one again gives the same reference and adds nothing.
        assert_eq!(kinds.define(Definition::Real), real);
        assert_eq!(kinds.definitions.len(), 7);

        // Each level has two Arrays of two scalars and one Real.
        assert_eq!(
            kinds.leaf_counts()[pair.0 as usize],
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
        let mut kind = kinds.define(Definition::Real);
        for _ in 0..40 {
            kind = kinds.define(Definition::Tuple(vec![kind, kind]));
        }
        assert_eq!(kinds.check(), Ok(()));
        assert_eq!(kinds.definitions.len(), 41);
        // A million million scalars, counted without writing any out.
        assert_eq!(kinds.leaf_counts()[kind.0 as usize], Some(1 << 40));
        // Comparing it with a copy in another table visits each pair once.
        assert!(kinds.same(kind, &kinds.clone(), kind));
    }

    #[test]
    fn the_same_kind_in_two_tables_is_recognized_whatever_its_place() {
        let mut ours = Kinds::default();
        let real = ours.define(Definition::Real);
        let ours_state = ours.define(Definition::Optional(real));
        let mut theirs = Kinds::default();
        theirs.define(Definition::Int);
        let real = theirs.define(Definition::Real);
        let theirs_state = theirs.define(Definition::Optional(real));
        assert_ne!(ours_state, theirs_state);
        assert!(ours.same(ours_state, &theirs, theirs_state));
        assert!(!ours.same(ours_state, &theirs, real));
    }

    #[test]
    fn a_struct_is_not_a_product_and_a_tuple_is_not_a_vector() {
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let members = named(&[("x", real), ("y", real)]);
        let product = kinds.define(Definition::Product(members.clone()));
        let structure = kinds.define(Definition::Struct(members));
        let vector = kinds.define(Definition::Vector {
            element: real,
            count: 2,
        });
        let tuple = kinds.define(Definition::Tuple(vec![real, real]));
        assert_eq!(kinds.check(), Ok(()));
        assert!(!kinds.same(product, &kinds, structure));
        assert!(!kinds.same(vector, &kinds, tuple));
        // Only the product and the vector may be the element of an Array.
        assert_eq!(kinds.fibers(), [true, true, false, true, false]);
        let array = kinds.define(Definition::Array {
            element: structure,
            axes: vec![AxisKind::Closed],
        });
        let defect = KindDefect::NotAFiber {
            at: array,
            names: structure,
        };
        assert_eq!(kinds.check(), Err(defect));
    }

    #[test]
    fn a_table_is_refused_for_each_broken_rule() {
        let table = |definitions: Vec<Definition>| Kinds { definitions }.check();
        let at = KindRef(1);
        let variant = |name: &str| (name.to_string(), None);

        // A kind may not refer to itself or to a later kind.
        let itself = table(vec![Definition::Real, Definition::Optional(KindRef(1))]);
        assert_eq!(itself, Err(KindDefect::NotYetStated { at, names: at }));
        // A sum needs two variants, with different names.
        let one = table(vec![
            Definition::Real,
            Definition::Sum(vec![variant("Only")]),
        ]);
        assert_eq!(one, Err(KindDefect::Malformed { at }));
        let twice = vec![variant("Same"), variant("Same")];
        let twice = table(vec![Definition::Real, Definition::Sum(twice)]);
        assert_eq!(twice, Err(KindDefect::Malformed { at }));
        // An Optional never directly holds an Optional.
        let optional = Definition::Optional(KindRef(0));
        let nested = table(vec![
            Definition::Real,
            optional.clone(),
            Definition::Optional(at),
        ]);
        assert_eq!(nested, Err(KindDefect::NestedOptional { at: KindRef(2) }));
        // A kind is stated once.
        let again = table(vec![Definition::Real, Definition::Real]);
        let first = KindRef(0);
        assert_eq!(again, Err(KindDefect::StatedTwice { first, again: at }));
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_answers_nothing() {
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let at = kinds.define(Definition::Vector {
            element: real,
            count: 3,
        });
        let facing = kinds.define(Definition::SO3);
        let pose = kinds.define(Definition::Product(named(&[
            ("at", at),
            ("facing", facing),
        ])));
        assert_eq!(kinds.check(), Ok(()));
        // The answer carries through anything built from it.
        assert_eq!(kinds.leaf_counts(), [Some(1), Some(3), None, None]);
        assert_eq!(kinds.leaves(pose), None);
        assert_eq!(kinds.leaves(at), Some(vec![Primitive::Real; 3]));
    }

    #[test]
    fn a_sum_stores_a_tag_then_every_payload() {
        let mut kinds = Kinds::default();
        let real = kinds.define(Definition::Real);
        let complex = kinds.define(Definition::Complex);
        let shape = kinds.define(Definition::Sum(vec![
            ("Point".to_string(), None),
            ("Segment".to_string(), Some(real)),
            ("Phase".to_string(), Some(complex)),
        ]));
        assert_eq!(kinds.check(), Ok(()));
        let expected = [
            Primitive::Int,
            Primitive::Real,
            Primitive::Real,
            Primitive::Real,
        ];
        assert_eq!(kinds.leaves(shape), Some(expected.to_vec()));
    }
}
