//! What a value is: its kind.
//!
//! Kinds are the language's own. This module writes down the ones that cross
//! a boundary so that every component names them the same way. The rules that
//! give an expression its kind are not here; they belong to analysis.
//!
//! A kind is logical. It says `Int`, not how many bits an Int has, and it says
//! nothing about where a value is stored. Those are choices a port states.
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
//! Gap: Structs, Tuples, Lists, Maps, Strings, the unit kind `None`, ranges,
//! boxes and views are kinds of the language that are not written here yet.
//! A Tuple, including one written `[T; k]`, is not a fiber.
//!
//! Gap: a kind is written out in full wherever it is used. That is harmless
//! for the kinds here but not for Structs nested inside one another with
//! Array members. Before those are added, a kind used in several places
//! needs to be stated once and referred to.

/// One scalar of a value, before any choice of width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Bool,
    Int,
    Real,
}

/// What one site of an Array holds: plain data of fixed width, with no
/// references and no size that is only known at run time.
///
/// A product or a sum is identified by its names, their order and their
/// fibers. The name a fiber was declared under is not part of it.
///
/// Gap: a product's declared algebra is also part of its identity and is not
/// represented here. Two products that differ only in algebra therefore
/// compare equal, which is wrong.
///
/// Gap: being able to write a fiber here does not make it valid. The
/// language admits a vector only as a real vector space, and has a distinct
/// dual that is not named here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fiber {
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
        element: Box<Fiber>,
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
    /// Named components in order. A component may be any fiber.
    Product(Vec<(String, Fiber)>),
    /// Exactly one of an ordered roster of named variants, each with a
    /// payload fiber or none.
    Sum(Vec<(String, Option<Fiber>)>),
}

impl Fiber {
    /// How many scalars one value of this fiber has. The answer is nothing
    /// if this module does not describe how the fiber is stored, or if the
    /// count does not fit in 64 bits.
    pub fn leaf_count(&self) -> Option<u64> {
        fn all<'a>(fibers: impl Iterator<Item = &'a Fiber>) -> Option<u64> {
            let mut sum = 0u64;
            for fiber in fibers {
                sum = sum.checked_add(fiber.leaf_count()?)?;
            }
            Some(sum)
        }
        match self {
            Fiber::Bool | Fiber::Int | Fiber::Real => Some(1),
            Fiber::Rational | Fiber::Complex => Some(2),
            Fiber::Color => Some(4),
            Fiber::Vector { element, count } => {
                element.leaf_count()?.checked_mul(u64::from(*count))
            }
            Fiber::Product(components) => all(components.iter().map(|(_, fiber)| fiber)),
            Fiber::Sum(variants) => {
                all(variants.iter().filter_map(|(_, payload)| payload.as_ref()))?.checked_add(1)
            }
            Fiber::SO2
            | Fiber::SO3
            | Fiber::Mat { .. }
            | Fiber::Form { .. }
            | Fiber::Metric { .. } => None,
        }
    }

    /// The scalars of one value, in order, or nothing if this module does not
    /// describe how the fiber is stored.
    ///
    /// A vector gives its positions first to last. A product gives its
    /// components as declared. A sum gives an Int tag holding the active
    /// variant's position, then the scalars of every variant's payload in
    /// declared order; the scalars of the variants that are not active hold
    /// zero.
    ///
    /// Gap: the storage of `SO2`, `SO3`, `Mat`, `Form` and `Metric` is not
    /// described. A port of such a fiber is refused as unsupported.
    pub fn leaves(&self) -> Option<Vec<Primitive>> {
        let mut leaves = Vec::new();
        self.push_leaves(&mut leaves)?;
        Some(leaves)
    }

    /// Whether this fiber obeys the rules the language states for its form:
    /// a vector has at least one component, and a sum has at least two
    /// variants with distinct names.
    pub fn well_formed(&self) -> bool {
        match self {
            Fiber::Vector { element, count } => *count >= 1 && element.well_formed(),
            Fiber::Product(components) => components.iter().all(|(_, fiber)| fiber.well_formed()),
            Fiber::Sum(variants) => {
                let distinct = variants.iter().enumerate().all(|(index, (name, _))| {
                    variants[..index].iter().all(|(other, _)| other != name)
                });
                variants.len() >= 2
                    && distinct
                    && variants
                        .iter()
                        .flat_map(|(_, payload)| payload)
                        .all(Fiber::well_formed)
            }
            _ => true,
        }
    }

    fn push_leaves(&self, leaves: &mut Vec<Primitive>) -> Option<()> {
        match self {
            Fiber::Bool => leaves.push(Primitive::Bool),
            Fiber::Int => leaves.push(Primitive::Int),
            Fiber::Real => leaves.push(Primitive::Real),
            Fiber::Rational => leaves.extend([Primitive::Int; 2]),
            Fiber::Complex => leaves.extend([Primitive::Real; 2]),
            Fiber::Color => leaves.extend([Primitive::Real; 4]),
            Fiber::Vector { element, count } => {
                for _ in 0..*count {
                    element.push_leaves(leaves)?;
                }
            }
            Fiber::Product(components) => {
                for (_, fiber) in components {
                    fiber.push_leaves(leaves)?;
                }
            }
            Fiber::Sum(variants) => {
                leaves.push(Primitive::Int);
                for (_, payload) in variants {
                    if let Some(fiber) = payload {
                        fiber.push_leaves(leaves)?;
                    }
                }
            }
            Fiber::SO2
            | Fiber::SO3
            | Fiber::Mat { .. }
            | Fiber::Form { .. }
            | Fiber::Metric { .. } => return None,
        }
        Some(())
    }
}

/// The form of one axis of an Array. It is part of the Array's kind, so two
/// Arrays whose axes differ in form are different kinds. A kind always states
/// the form of every axis; a written type that states only a rank means every
/// axis is closed.
///
/// Gap: how the stored part of an axis that is open at an end is described,
/// given that only a finite part can be stored, is not written yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// What a value is.
///
/// Two kinds are the same exactly when they are equal here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A single value of a fiber.
    Fiber(Fiber),
    /// One fiber at every site of a domain. The kind states how many axes the
    /// domain has and the form of each. Where each axis starts and how far it
    /// extends is not part of the kind.
    Array { element: Fiber, axes: Vec<AxisKind> },
    /// A value of the inner kind, or absence. Optional kinds do not nest:
    /// "absent or absent or a value" is the same as "absent or a value".
    Optional(Box<Kind>),
}

impl Kind {
    /// How many scalars one element of this kind has, or nothing if its
    /// fiber's storage is not described.
    pub fn leaf_count(&self) -> Option<u64> {
        match self {
            Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fiber.leaf_count(),
            Kind::Optional(inner) => inner.leaf_count(),
        }
    }

    /// The scalars of one element of this kind, in order, or nothing if its
    /// fiber's storage is not described. A port lists one leaf for each, in
    /// this order.
    pub fn leaves(&self) -> Option<Vec<Primitive>> {
        match self {
            Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fiber.leaves(),
            Kind::Optional(inner) => inner.leaves(),
        }
    }

    /// The run-time facts of a value of this kind, in order: presence first
    /// for an Optional, then the sizes of what it holds. A port lists one
    /// entry for each, in this order.
    pub fn shape_fields(&self) -> Vec<ShapeField> {
        match self {
            Kind::Fiber(_) => Vec::new(),
            Kind::Array { axes, .. } => vec![ShapeField::Domain { rank: axes.len() }],
            Kind::Optional(inner) => {
                let mut shape_fields = vec![ShapeField::Presence];
                shape_fields.extend(inner.shape_fields());
                shape_fields
            }
        }
    }

    /// Whether every fiber in this kind is well formed.
    pub fn well_formed(&self) -> bool {
        match self {
            Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fiber.well_formed(),
            Kind::Optional(inner) => inner.well_formed(),
        }
    }

    /// Whether an Optional directly holds another Optional anywhere in this
    /// kind.
    pub fn nests_optional(&self) -> bool {
        match self {
            Kind::Optional(inner) => matches!(**inner, Kind::Optional(_)) || inner.nests_optional(),
            Kind::Fiber(_) | Kind::Array { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conserved() -> Fiber {
        let real = |name: &str| (name.to_string(), Fiber::Real);
        Fiber::Product(vec![real("rho"), real("mom"), real("nrg")])
    }

    #[test]
    fn an_optional_array_of_a_product_has_the_products_leaves_and_one_domain() {
        let field = |axis| Kind::Array {
            element: conserved(),
            axes: vec![axis],
        };
        let state = Kind::Optional(Box::new(field(AxisKind::Closed)));
        assert_eq!(state.leaves(), Some(vec![Primitive::Real; 3]));
        let facts = vec![ShapeField::Presence, ShapeField::Domain { rank: 1 }];
        assert_eq!(state.shape_fields(), facts);
        assert!(!state.nests_optional());
        assert!(Kind::Optional(Box::new(state.clone())).nests_optional());
        // The form of an axis is part of the kind.
        assert_ne!(field(AxisKind::Closed), field(AxisKind::Periodic));
    }

    #[test]
    fn a_product_is_identified_by_component_names_order_and_fibers() {
        let pair = |a: &str, b: &str| {
            Fiber::Product(vec![
                (a.to_string(), Fiber::Real),
                (b.to_string(), Fiber::Int),
            ])
        };
        assert_eq!(pair("x", "n"), pair("x", "n"));
        assert_ne!(pair("x", "n"), pair("y", "n"));
        let swapped = Fiber::Product(vec![
            ("n".to_string(), Fiber::Int),
            ("x".to_string(), Fiber::Real),
        ]);
        assert_ne!(pair("x", "n"), swapped);
    }

    #[test]
    fn a_vector_component_contributes_its_elements_in_order() {
        let velocity = Fiber::Vector {
            element: Box::new(Fiber::Real),
            count: 3,
        };
        let particle = Fiber::Product(vec![
            ("alive".to_string(), Fiber::Bool),
            ("vel".to_string(), velocity),
            ("id".to_string(), Fiber::Int),
        ]);
        let expected = [
            Primitive::Bool,
            Primitive::Real,
            Primitive::Real,
            Primitive::Real,
            Primitive::Int,
        ];
        assert_eq!(particle.leaves(), Some(expected.to_vec()));
        assert_eq!(particle.leaf_count(), Some(5));
    }

    #[test]
    fn a_sum_stores_a_tag_then_every_payload() {
        let shape = Fiber::Sum(vec![
            ("Point".to_string(), None),
            ("Segment".to_string(), Some(Fiber::Real)),
            ("Phase".to_string(), Some(Fiber::Complex)),
        ]);
        let expected = [
            Primitive::Int,
            Primitive::Real,
            Primitive::Real,
            Primitive::Real,
        ];
        assert_eq!(shape.leaves(), Some(expected.to_vec()));
        assert!(shape.well_formed());

        // A roster needs two variants, and their names must differ.
        let variant = |name: &str| (name.to_string(), None);
        assert!(!Fiber::Sum(vec![variant("Only")]).well_formed());
        assert!(!Fiber::Sum(vec![variant("Same"), variant("Same")]).well_formed());
    }

    #[test]
    fn a_fiber_whose_storage_is_not_described_answers_nothing() {
        assert_eq!(Fiber::SO3.leaves(), None);
        // The answer carries through anything built from it.
        let pose = Fiber::Product(vec![
            (
                "at".to_string(),
                Fiber::Vector {
                    element: Box::new(Fiber::Real),
                    count: 3,
                },
            ),
            ("facing".to_string(), Fiber::SO3),
        ]);
        assert_eq!(pose.leaves(), None);
        assert_eq!(pose.leaf_count(), None);
        // It is still a kind with an identity.
        assert_eq!(pose, pose.clone());
        assert_ne!(Fiber::SO3, Fiber::SO2);
    }
}
