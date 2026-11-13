//! What a value is: its kind.
//!
//! Kinds are the language's own. This module writes down the ones that cross
//! a boundary so that every component names them the same way. The rules that
//! give an expression its kind are not here; they belong to analysis.
//!
//! A kind is logical. It says `Int`, not how many bits an Int has, and it says
//! nothing about where a value is stored. Those are choices a port states.
//!
//! Gap: Sum fibers, Structs, Tuples, Lists, Maps and Strings are kinds of the
//! language that are not written here yet.

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
/// Two fibers are the same exactly when they are equal here. A product is
/// identified by its component names, their order and their fibers. The name
/// a product or any other fiber was declared under is not part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fiber {
    Bool,
    Int,
    Real,
    /// A stated number of components of one fiber, at least one.
    Vector {
        element: Box<Fiber>,
        count: u32,
    },
    /// Named components in order. A component may be any fiber, including a
    /// vector or another product.
    Product(Vec<(String, Fiber)>),
}

impl Fiber {
    /// How many scalars one value of this fiber has, or nothing if the count
    /// does not fit in 64 bits.
    pub fn leaf_count(&self) -> Option<u64> {
        match self {
            Fiber::Bool | Fiber::Int | Fiber::Real => Some(1),
            Fiber::Vector { element, count } => {
                element.leaf_count()?.checked_mul(u64::from(*count))
            }
            Fiber::Product(components) => components
                .iter()
                .try_fold(0u64, |sum, (_, fiber)| sum.checked_add(fiber.leaf_count()?)),
        }
    }

    /// The scalars of one value, in order: a vector's elements first to last,
    /// a product's components as declared.
    pub fn leaves(&self) -> Vec<Primitive> {
        let mut leaves = Vec::new();
        self.push_leaves(&mut leaves);
        leaves
    }

    fn push_leaves(&self, leaves: &mut Vec<Primitive>) {
        match self {
            Fiber::Bool => leaves.push(Primitive::Bool),
            Fiber::Int => leaves.push(Primitive::Int),
            Fiber::Real => leaves.push(Primitive::Real),
            Fiber::Vector { element, count } => {
                for _ in 0..*count {
                    element.push_leaves(leaves);
                }
            }
            Fiber::Product(components) => {
                for (_, fiber) in components {
                    fiber.push_leaves(leaves);
                }
            }
        }
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
    /// How many scalars one element of this kind has.
    pub fn leaf_count(&self) -> Option<u64> {
        match self {
            Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fiber.leaf_count(),
            Kind::Optional(inner) => inner.leaf_count(),
        }
    }

    /// The scalars of one element of this kind, in order. A port lists one
    /// leaf for each, in this order.
    pub fn leaves(&self) -> Vec<Primitive> {
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
        assert_eq!(state.leaves(), vec![Primitive::Real; 3]);
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
        assert_eq!(particle.leaves(), expected);
        assert_eq!(particle.leaf_count(), Some(5));
    }
}
