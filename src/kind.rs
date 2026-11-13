//! What a value is: its kind.
//!
//! Kinds are the language's own. This module writes down the ones that cross
//! a boundary so that every component names them the same way. The rules that
//! give an expression its kind are not here; they belong to analysis.
//!
//! A kind is logical. It says `Int`, not how many bits an Int has, and it says
//! nothing about where a value is stored. Those are choices a port states.
//!
//! Every fiber of the language is named here, so each has one definition.
//! Naming a fiber does not oblige any component to support it. Where stating
//! how a fiber is stored would be costly, this module declines to: it answers
//! "not described" and a component is free to report the fiber as unsupported.
//!
//! Gap: Structs, Tuples, Lists, Maps and Strings are kinds of the language
//! that are not written here yet.

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
/// Two fibers are the same exactly when they are equal here. A product or a
/// sum is identified by its names, their order and their fibers. The name a
/// fiber was declared under is not part of it.
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
    /// A static list of a stated number of positions, all of one fiber.
    Repeat {
        element: Box<Fiber>,
        count: u32,
    },
    /// A static list whose positions each state their own fiber.
    Positions(Vec<Fiber>),
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
            Fiber::Vector { element, count } | Fiber::Repeat { element, count } => {
                element.leaf_count()?.checked_mul(u64::from(*count))
            }
            Fiber::Positions(positions) => all(positions.iter()),
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
    /// A vector or static list gives its positions first to last. A product
    /// gives its components as declared. A sum gives an Int tag holding the
    /// active variant's position, then the scalars of every variant's payload
    /// in declared order.
    ///
    /// Gap: the storage of `SO2`, `SO3`, `Mat`, `Form` and `Metric` is not
    /// described. A port of such a fiber is refused as unsupported.
    pub fn leaves(&self) -> Option<Vec<Primitive>> {
        let mut leaves = Vec::new();
        self.push_leaves(&mut leaves)?;
        Some(leaves)
    }

    fn push_leaves(&self, leaves: &mut Vec<Primitive>) -> Option<()> {
        match self {
            Fiber::Bool => leaves.push(Primitive::Bool),
            Fiber::Int => leaves.push(Primitive::Int),
            Fiber::Real => leaves.push(Primitive::Real),
            Fiber::Rational => leaves.extend([Primitive::Int; 2]),
            Fiber::Complex => leaves.extend([Primitive::Real; 2]),
            Fiber::Color => leaves.extend([Primitive::Real; 4]),
            Fiber::Vector { element, count } | Fiber::Repeat { element, count } => {
                for _ in 0..*count {
                    element.push_leaves(leaves)?;
                }
            }
            Fiber::Positions(positions) => {
                for fiber in positions {
                    fiber.push_leaves(leaves)?;
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
/// Arrays whose axes differ in form are different kinds.
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

/// A kind of size that is only known at run time.
///
/// A value's kind fixes its structure at compile time, including the form of
/// each Array axis. What compile time cannot fix is where an axis starts and
/// how many elements there are. Those are the dimensions.
///
/// The presence of an Optional value is also known only at run time, but it
/// is not a dimension. It is metadata of a value: it counts nothing, and an
/// absent value is not an empty one.
///
/// Gap: how a port and a value carry presence is not written yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    /// The number of elements in a List.
    Length,
    /// The domain of an Array: an origin and an extent for each axis.
    Domain { rank: u8 },
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
    /// A value of the inner kind, or absence.
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

    /// The run-time sizes of a value of this kind, in order. A port lists one
    /// size for each, in this order.
    pub fn dimensions(&self) -> Vec<Dimension> {
        match self {
            Kind::Fiber(_) => Vec::new(),
            Kind::Array { axes, .. } => vec![Dimension::Domain {
                rank: axes.len() as u8,
            }],
            Kind::Optional(inner) => inner.dimensions(),
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
        assert_eq!(state.dimensions(), vec![Dimension::Domain { rank: 1 }]);
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
