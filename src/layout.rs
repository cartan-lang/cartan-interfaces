//! How a value sits in storage: its layout.
//!
//! A kind says what a value is. A layout says where its scalars are kept. One
//! kind can have many layouts: an Array of a product may keep whole products
//! side by side in one plane, keep each component in a plane of its own, or
//! mix the two.
//!
//! A layout follows the structure of its kind and is stated in the same way:
//! once, in a table, in terms of references to the layouts it is built from.
//! A layout used at many places in a value appears once.
//!
//! A layout does not name storage. It numbers the planes it uses from zero
//! and places its parts among them, so the same layout serves every value
//! laid out that way. A port supplies the actual storage, one slot for each
//! plane.
//!
//! A plane is a run of equal-sized elements. Within one element, a scalar
//! sits at a byte offset. A layout says, for each of its planes, which bytes
//! of an element it uses.

use crate::kind::{Definition, KindRef, Kinds, Primitive};
use std::collections::{HashMap, HashSet};

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

impl Scalar {
    /// Whether this scalar can store the given primitive.
    pub fn stores(self, primitive: Primitive) -> bool {
        match primitive {
            Primitive::Bool => self == Scalar::I32,
            Primitive::Int => matches!(self, Scalar::I32 | Scalar::I64),
            Primitive::Real => matches!(self, Scalar::F32 | Scalar::F64),
        }
    }

    pub fn bytes(self) -> u32 {
        match self {
            Scalar::I32 | Scalar::F32 => 4,
            Scalar::I64 | Scalar::F64 => 8,
        }
    }
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

impl Layouts {
    /// The reference for a layout, adding it to the table only if it is not
    /// already there.
    pub fn define(&mut self, layout: Layout) -> LayoutRef {
        let found = self.definitions.iter().position(|known| *known == layout);
        let place = found.unwrap_or_else(|| {
            self.definitions.push(layout);
            self.definitions.len() - 1
        });
        LayoutRef(place as u32)
    }

    pub fn get(&self, layout: LayoutRef) -> Option<&Layout> {
        self.definitions.get(layout.0 as usize)
    }

    fn children(layout: &Layout) -> Vec<LayoutRef> {
        match layout {
            Layout::Scalar(_) => Vec::new(),
            Layout::Group(parts) => parts.iter().map(|part| part.layout).collect(),
            Layout::Repeat { part, .. } => vec![*part],
            Layout::Array(inner) | Layout::Optional(inner) => vec![*inner],
        }
    }

    /// Check the table and work out what follows from each layout. No layout
    /// may use more than `most_planes` planes, which is the number of pieces
    /// of storage there are to put them in.
    pub fn measure(&self, most_planes: u32) -> Result<Measures, LayoutDefect> {
        let mut measures = Measures {
            planes: Vec::new(),
            facts: Vec::new(),
            spans: Vec::new(),
        };
        let mut seen = HashSet::new();
        for (index, layout) in self.definitions.iter().enumerate() {
            let at = LayoutRef(index as u32);
            let too_large = LayoutDefect::TooLarge { at };
            for names in Self::children(layout) {
                if names.0 as usize >= index {
                    return Err(LayoutDefect::NotYetStated { at, names });
                }
            }
            let (facts, spans) = match layout {
                Layout::Scalar(scalar) => (0, vec![(0, scalar.bytes())]),
                Layout::Group(parts) => self.group(at, parts, &measures, most_planes)?,
                Layout::Repeat {
                    part,
                    count,
                    plane_step,
                    offset_step,
                } => {
                    let inner = &measures.spans[part.0 as usize];
                    let facts = measures.facts[part.0 as usize]
                        .checked_mul(u64::from(*count))
                        .ok_or(too_large)?;
                    let spans = repeat(inner, *count, *plane_step, *offset_step, most_planes);
                    (
                        facts,
                        spans.map_err(|overlap| {
                            if overlap {
                                LayoutDefect::CopiesOverlap { at }
                            } else {
                                too_large
                            }
                        })?,
                    )
                }
                Layout::Array(element) => {
                    if measures.facts[element.0 as usize] != 0 {
                        return Err(LayoutDefect::FactsInElement { at });
                    }
                    (1, measures.spans[element.0 as usize].clone())
                }
                Layout::Optional(inner) => {
                    let facts = measures.facts[inner.0 as usize].checked_add(1);
                    (
                        facts.ok_or(too_large)?,
                        measures.spans[inner.0 as usize].clone(),
                    )
                }
            };
            if !seen.insert(layout) {
                let first = self.definitions.iter().position(|known| known == layout);
                return Err(LayoutDefect::StatedTwice {
                    first: LayoutRef(first.unwrap_or(index) as u32),
                    again: at,
                });
            }
            measures.planes.push(spans.len() as u32);
            measures.facts.push(facts);
            measures.spans.push(spans);
        }
        Ok(measures)
    }

    fn group(
        &self,
        at: LayoutRef,
        parts: &[Part],
        measures: &Measures,
        most_planes: u32,
    ) -> Result<(u64, Vec<(u32, u32)>), LayoutDefect> {
        let too_large = LayoutDefect::TooLarge { at };
        let mut facts = 0u64;
        let mut placed: Vec<(u32, u32, u32, u32)> = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let place = part.layout.0 as usize;
            facts = facts.checked_add(measures.facts[place]).ok_or(too_large)?;
            for (plane, (start, end)) in measures.spans[place].iter().enumerate() {
                let plane = part.plane.0.checked_add(plane as u32).ok_or(too_large)?;
                let start = start.checked_add(part.offset).ok_or(too_large)?;
                let end = end.checked_add(part.offset).ok_or(too_large)?;
                if plane >= most_planes {
                    return Err(too_large);
                }
                placed.push((plane, start, end, index as u32));
            }
        }
        placed.sort_unstable();
        for pair in placed.windows(2) {
            let ((plane, _, end, first), (next, start, _, second)) = (pair[0], pair[1]);
            if plane == next && start < end && first != second {
                let (first, second) = (first.min(second), first.max(second));
                return Err(LayoutDefect::PartsOverlap { at, first, second });
            }
        }
        let planes = placed
            .iter()
            .map(|(plane, ..)| plane + 1)
            .max()
            .unwrap_or(0);
        let mut spans = vec![(u32::MAX, 0u32); planes as usize];
        for (plane, start, end, _) in placed {
            let span = &mut spans[plane as usize];
            *span = (span.0.min(start), span.1.max(end));
        }
        // A plane no part uses still belongs to the group; it spans nothing.
        for span in &mut spans {
            if span.0 == u32::MAX {
                *span = (0, 0);
            }
        }
        Ok((facts, spans))
    }

    /// Whether a layout lays out a kind: the same structure, part for part,
    /// with every scalar stored in a way that can hold it. Both tables must
    /// be checked. Each pairing of a kind with a layout is examined once,
    /// however often it is shared.
    ///
    /// A kind whose storage is not described fits no layout.
    pub fn fits(&self, layout: LayoutRef, kinds: &Kinds, kind: KindRef) -> bool {
        self.fits_with(layout, kinds, kind, &mut HashMap::new())
    }

    fn fits_with(
        &self,
        layout: LayoutRef,
        kinds: &Kinds,
        kind: KindRef,
        known: &mut HashMap<(LayoutRef, KindRef), bool>,
    ) -> bool {
        if let Some(answer) = known.get(&(layout, kind)) {
            return *answer;
        }
        let (Some(how), Some(what)) = (self.get(layout), kinds.get(kind)) else {
            return false;
        };
        let scalar = |layout: LayoutRef, primitive: Primitive| matches!(self.get(layout), Some(Layout::Scalar(scalar)) if scalar.stores(primitive));
        // What a group or a repeat must hold, one entry for each part.
        enum Holds {
            Kind(KindRef),
            Scalar(Primitive),
        }
        let members: Option<Vec<Holds>> = match what {
            Definition::Rational => Some(vec![
                Holds::Scalar(Primitive::Int),
                Holds::Scalar(Primitive::Int),
            ]),
            Definition::Complex => Some((0..2).map(|_| Holds::Scalar(Primitive::Real)).collect()),
            Definition::Color => Some((0..4).map(|_| Holds::Scalar(Primitive::Real)).collect()),
            Definition::Product(members) | Definition::Struct(members) => {
                Some(members.iter().map(|(_, kind)| Holds::Kind(*kind)).collect())
            }
            Definition::Tuple(positions) => {
                Some(positions.iter().map(|kind| Holds::Kind(*kind)).collect())
            }
            Definition::Sum(variants) => {
                let payloads = variants.iter().filter_map(|(_, payload)| *payload);
                let tag = std::iter::once(Holds::Scalar(Primitive::Int));
                Some(tag.chain(payloads.map(Holds::Kind)).collect())
            }
            _ => None,
        };
        let mut holds = |this: &Self, layout: LayoutRef, held: &Holds| match held {
            Holds::Scalar(primitive) => scalar(layout, *primitive),
            Holds::Kind(kind) => this.fits_with(layout, kinds, *kind, known),
        };
        let answer = match (how, what) {
            (Layout::Scalar(scalar), Definition::Bool) => scalar.stores(Primitive::Bool),
            (Layout::Scalar(scalar), Definition::Int) => scalar.stores(Primitive::Int),
            (Layout::Scalar(scalar), Definition::Real) => scalar.stores(Primitive::Real),
            (Layout::Array(element), Definition::Array { element: kind, .. }) => {
                self.fits_with(*element, kinds, *kind, known)
            }
            (Layout::Optional(inner), Definition::Optional(kind)) => {
                self.fits_with(*inner, kinds, *kind, known)
            }
            (
                Layout::Repeat { part, count, .. },
                Definition::Vector {
                    element,
                    count: width,
                },
            ) => count == width && self.fits_with(*part, kinds, *element, known),
            (Layout::Group(parts), Definition::Vector { element, count }) => {
                parts.len() as u64 == u64::from(*count)
                    && parts
                        .iter()
                        .all(|part| holds(self, part.layout, &Holds::Kind(*element)))
            }
            (Layout::Group(parts), _) => match &members {
                Some(members) => {
                    parts.len() == members.len()
                        && parts
                            .iter()
                            .zip(members)
                            .all(|(part, held)| holds(self, part.layout, held))
                }
                None => false,
            },
            (Layout::Repeat { part, count, .. }, _) => match &members {
                // A repeat stands for a group whose parts are all alike.
                Some(members) => {
                    *count as usize == members.len()
                        && members.iter().all(|held| holds(self, *part, held))
                }
                None => false,
            },
            _ => false,
        };
        known.insert((layout, kind), answer);
        answer
    }
}

/// The spans of `count` copies of a layout with the given spans, each copy
/// a fixed step on from the last. The error says whether the copies overlap;
/// otherwise something did not fit.
fn repeat(
    inner: &[(u32, u32)],
    count: u32,
    plane_step: u32,
    offset_step: u32,
    most_planes: u32,
) -> Result<Vec<(u32, u32)>, bool> {
    if count == 0 || inner.is_empty() {
        return Ok(Vec::new());
    }
    let last = count - 1;
    if plane_step == 0 {
        // The copies share their planes, so each must clear the one before.
        let clear = inner.iter().all(|(start, end)| offset_step >= end - start);
        if count > 1 && !clear {
            return Err(true);
        }
        let reach = last.checked_mul(offset_step).ok_or(false)?;
        return inner
            .iter()
            .map(|(start, end)| Ok((*start, end.checked_add(reach).ok_or(false)?)))
            .collect();
    }
    // The copies take separate planes.
    if count > 1 && (plane_step as usize) < inner.len() {
        return Err(true);
    }
    let planes = last
        .checked_mul(plane_step)
        .and_then(|reach| reach.checked_add(inner.len() as u32))
        .filter(|planes| *planes <= most_planes)
        .ok_or(false)?;
    let mut spans = vec![(0u32, 0u32); planes as usize];
    for copy in 0..count {
        let shift = copy.checked_mul(offset_step).ok_or(false)?;
        for (plane, (start, end)) in inner.iter().enumerate() {
            let place = (copy * plane_step) as usize + plane;
            spans[place] = (
                start.checked_add(shift).ok_or(false)?,
                end.checked_add(shift).ok_or(false)?,
            );
        }
    }
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::AxisKind;

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
        let real = kinds.define(Definition::Real);
        let int = kinds.define(Definition::Int);
        let members = vec![("mass".to_string(), real), ("id".to_string(), int)];
        let particle = kinds.define(Definition::Product(members));
        let field = kinds.define(Definition::Array {
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
        let real = kinds.define(Definition::Real);
        let inner = kinds.define(Definition::Vector {
            element: real,
            count: 1000,
        });
        let outer = kinds.define(Definition::Vector {
            element: inner,
            count: 1000,
        });
        let field = kinds.define(Definition::Array {
            element: outer,
            axes: vec![AxisKind::Closed],
        });

        let mut layouts = Layouts::default();
        let scalar = layouts.define(Layout::Scalar(Scalar::F32));
        let row = layouts.define(Layout::Repeat {
            part: scalar,
            count: 1000,
            plane_step: 0,
            offset_step: 4,
        });
        let grid = layouts.define(Layout::Repeat {
            part: row,
            count: 1000,
            plane_step: 0,
            offset_step: 4000,
        });
        let array = layouts.define(Layout::Array(grid));

        // A million scalars in each element, in four definitions.
        assert_eq!(layouts.definitions.len(), 4);
        let measures = layouts.measure(1).unwrap();
        assert_eq!(measures.spans[array.0 as usize], [(0, 4_000_000)]);
        assert!(layouts.fits(array, &kinds, field));
    }

    #[test]
    fn a_struct_keeps_each_array_member_in_its_own_planes_with_its_own_domain() {
        let (mut kinds, field) = particles();
        let real = kinds.define(Definition::Real);
        let level = vec![
            ("cells".to_string(), field),
            ("ghosts".to_string(), field),
            ("time".to_string(), real),
        ];
        let level = kinds.define(Definition::Struct(level));
        let mesh = vec![("coarse".to_string(), level), ("fine".to_string(), level)];
        let mesh = kinds.define(Definition::Struct(mesh));

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
        let real = kinds.define(Definition::Real);
        let complex = kinds.define(Definition::Complex);
        let shape = kinds.define(Definition::Sum(vec![
            ("Point".to_string(), None),
            ("Segment".to_string(), Some(real)),
            ("Phase".to_string(), Some(complex)),
        ]));

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
