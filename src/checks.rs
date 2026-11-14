//! Every `impl` of the contract types: accessors and the checks that a
//! description is well formed.

use super::*;
use std::collections::{HashMap, HashSet};

// Kinds: what a value is

impl Field {
    /// How many scalars one element of the field has.
    pub fn scalars(self) -> u64 {
        match self {
            Field::Real => 1,
            Field::Rational | Field::Complex => 2,
        }
    }
}

impl Kinds {
    /// The reference for a fiber, adding it to the table only if it is not
    /// already there.
    pub fn fiber(&mut self, fiber: Fiber) -> FiberRef {
        let found = self.fibers.iter().position(|known| *known == fiber);
        let place = found.unwrap_or_else(|| {
            self.fibers.push(fiber);
            self.fibers.len() - 1
        });
        FiberRef(place as u32)
    }

    /// The reference for a kind, adding it to the table only if it is not
    /// already there.
    pub fn define(&mut self, kind: Kind) -> KindRef {
        let found = self.kinds.iter().position(|known| *known == kind);
        let place = found.unwrap_or_else(|| {
            self.kinds.push(kind);
            self.kinds.len() - 1
        });
        KindRef(place as u32)
    }

    pub fn get(&self, kind: KindRef) -> Option<&Kind> {
        self.kinds.get(kind.0 as usize)
    }

    pub fn get_fiber(&self, fiber: FiberRef) -> Option<&Fiber> {
        self.fibers.get(fiber.0 as usize)
    }

    /// The fibers a fiber is built from.
    fn parts(fiber: &Fiber) -> Vec<FiberRef> {
        match fiber {
            Fiber::Product(members) => members.iter().map(|(_, fiber)| *fiber).collect(),
            Fiber::Sum(variants) => variants.iter().filter_map(|(_, fiber)| *fiber).collect(),
            _ => Vec::new(),
        }
    }

    /// The kinds a kind is built from.
    fn children(kind: &Kind) -> Vec<KindRef> {
        match kind {
            Kind::Optional(inner) => vec![*inner],
            Kind::Struct(members) => members.iter().map(|(_, kind)| *kind).collect(),
            Kind::Tuple(positions) => positions.clone(),
            Kind::Fiber(_) | Kind::Array { .. } => Vec::new(),
        }
    }

    /// Check that the tables are well formed. Checked tables state each
    /// fiber and each kind once, so within them equal references mean the
    /// same thing.
    pub fn check(&self) -> Result<(), KindDefect> {
        let mut seen = HashSet::new();
        for (index, fiber) in self.fibers.iter().enumerate() {
            let at = FiberRef(index as u32);
            for names in Self::parts(fiber) {
                if names.0 as usize >= index {
                    return Err(KindDefect::FiberNotYetStated { at, names });
                }
            }
            let malformed = match fiber {
                Fiber::Vector { count, .. } => *count == 0,
                Fiber::Sum(variants) => {
                    let mut names = HashSet::new();
                    variants.len() < 2 || !variants.iter().all(|(name, _)| names.insert(name))
                }
                _ => false,
            };
            if malformed {
                return Err(KindDefect::Malformed { at });
            }
            if !seen.insert(fiber) {
                let first = self.fibers.iter().position(|known| known == fiber);
                return Err(KindDefect::FiberStatedTwice {
                    first: FiberRef(first.unwrap_or(index) as u32),
                    again: at,
                });
            }
        }
        let mut seen = HashSet::new();
        for (index, kind) in self.kinds.iter().enumerate() {
            let at = KindRef(index as u32);
            if let Kind::Fiber(names) | Kind::Array { element: names, .. } = kind {
                if names.0 as usize >= self.fibers.len() {
                    let names = *names;
                    return Err(KindDefect::UnknownFiber { at, names });
                }
            }
            for names in Self::children(kind) {
                if names.0 as usize >= index {
                    return Err(KindDefect::NotYetStated { at, names });
                }
            }
            if let Kind::Optional(inner) = kind {
                if matches!(self.kinds[inner.0 as usize], Kind::Optional(_)) {
                    return Err(KindDefect::NestedOptional { at });
                }
            }
            if !seen.insert(kind) {
                let first = self.kinds.iter().position(|known| known == kind);
                return Err(KindDefect::StatedTwice {
                    first: KindRef(first.unwrap_or(index) as u32),
                    again: at,
                });
            }
        }
        Ok(())
    }

    /// How many scalars each fiber has, one answer per fiber. The answer is
    /// nothing if the fiber's storage is not described, or if the count does
    /// not fit in 64 bits.
    ///
    /// Gap: the storage of `SO2`, `SO3`, `Mat`, `Form` and `Metric` is not
    /// described. A port of a kind that includes one is refused as
    /// unsupported.
    fn fiber_scalars(&self) -> Vec<Option<u64>> {
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.fibers.len());
        for fiber in &self.fibers {
            let parts = Self::parts(fiber)
                .iter()
                .try_fold(0u64, |sum, part| sum.checked_add(counts[part.0 as usize]?));
            let count = match fiber {
                Fiber::Bool | Fiber::Int | Fiber::Real => Some(1),
                Fiber::Rational | Fiber::Complex => Some(2),
                Fiber::Color => Some(4),
                Fiber::Vector { over, count } => over.scalars().checked_mul(u64::from(*count)),
                Fiber::Sum(_) => parts.and_then(|payloads| payloads.checked_add(1)),
                Fiber::Product(_) => parts,
                Fiber::SO2
                | Fiber::SO3
                | Fiber::Mat { .. }
                | Fiber::Form { .. }
                | Fiber::Metric { .. } => None,
            };
            counts.push(count);
        }
        counts
    }

    /// How many scalars one element of each kind has, one answer per kind,
    /// worked out once for the whole table. The answer is nothing if the
    /// kind includes a fiber whose storage is not described, or if the count
    /// does not fit in 64 bits.
    ///
    /// The tables must be checked first.
    pub fn scalar_counts(&self) -> Vec<Option<u64>> {
        let fibers = self.fiber_scalars();
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.kinds.len());
        for kind in &self.kinds {
            let count = match kind {
                Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fibers[fiber.0 as usize],
                _ => Self::children(kind)
                    .iter()
                    .try_fold(0u64, |sum, kind| sum.checked_add(counts[kind.0 as usize]?)),
            };
            counts.push(count);
        }
        counts
    }

    /// How many run-time facts a value of each kind has, one answer per
    /// kind, worked out once for the whole table. The answer is nothing if
    /// the count does not fit in 64 bits.
    pub fn shape_field_counts(&self) -> Vec<Option<u64>> {
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.kinds.len());
        for kind in &self.kinds {
            let members = |counts: &Vec<Option<u64>>| {
                Self::children(kind)
                    .iter()
                    .try_fold(0u64, |sum, kind| sum.checked_add(counts[kind.0 as usize]?))
            };
            let count = match kind {
                Kind::Fiber(_) => Some(0),
                Kind::Array { .. } => Some(1),
                Kind::Optional(_) => members(&counts).and_then(|inner| inner.checked_add(1)),
                Kind::Struct(_) | Kind::Tuple(_) => members(&counts),
            };
            counts.push(count);
        }
        counts
    }

    /// The run-time facts of a value of a kind, in order: presence first for
    /// an Optional, a domain for an Array, and the facts of each member of a
    /// Struct or Tuple in turn. A port lists one entry for each, in this
    /// order.
    ///
    /// This writes the facts of a repeated kind out in full. Ask
    /// `shape_field_counts` first. Parts of a kind that have no facts are
    /// not visited.
    pub fn shape_fields(&self, kind: KindRef) -> Vec<ShapeField> {
        let counts = self.shape_field_counts();
        let mut fields = Vec::new();
        self.push_shape_fields(kind, &counts, &mut fields);
        fields
    }

    fn push_shape_fields(
        &self,
        kind: KindRef,
        counts: &[Option<u64>],
        fields: &mut Vec<ShapeField>,
    ) {
        if counts.get(kind.0 as usize) == Some(&Some(0)) {
            return;
        }
        match self.get(kind) {
            Some(Kind::Array { axes, .. }) => fields.push(ShapeField::Domain { rank: axes.len() }),
            Some(Kind::Optional(inner)) => {
                fields.push(ShapeField::Presence);
                self.push_shape_fields(*inner, counts, fields);
            }
            Some(kind @ (Kind::Struct(_) | Kind::Tuple(_))) => {
                for member in Self::children(kind) {
                    self.push_shape_fields(member, counts, fields);
                }
            }
            Some(Kind::Fiber(_)) | None => {}
        }
    }

    /// Whether a kind in these tables and a kind in other tables are the
    /// same kind. Both must be checked. Each pair of kinds, and each pair of
    /// fibers, is compared once, however often it is shared.
    pub fn same(&self, kind: KindRef, other: &Kinds, theirs: KindRef) -> bool {
        let mut known = Compared::default();
        self.same_kind(kind, other, theirs, &mut known)
    }

    fn same_kind(
        &self,
        kind: KindRef,
        other: &Kinds,
        theirs: KindRef,
        known: &mut Compared,
    ) -> bool {
        if let Some(answer) = known.kinds.get(&(kind, theirs)) {
            return *answer;
        }
        let (Some(mine), Some(yours)) = (self.get(kind), other.get(theirs)) else {
            return false;
        };
        let names = |members: &[(String, KindRef)]| -> Vec<String> {
            members.iter().map(|(name, _)| name.clone()).collect()
        };
        let alike = match (mine, yours) {
            (Kind::Fiber(a), Kind::Fiber(b)) => self.same_fiber(*a, other, *b, known),
            (
                Kind::Array { element: a, axes },
                Kind::Array {
                    element: b,
                    axes: theirs,
                },
            ) => axes == theirs && self.same_fiber(*a, other, *b, known),
            (Kind::Optional(_), Kind::Optional(_)) | (Kind::Tuple(_), Kind::Tuple(_)) => true,
            (Kind::Struct(a), Kind::Struct(b)) => names(a) == names(b),
            _ => false,
        };
        let (a, b) = (Self::children(mine), Self::children(yours));
        let answer = alike
            && a.len() == b.len()
            && a.iter()
                .zip(&b)
                .all(|(a, b)| self.same_kind(*a, other, *b, known));
        known.kinds.insert((kind, theirs), answer);
        answer
    }

    fn same_fiber(
        &self,
        fiber: FiberRef,
        other: &Kinds,
        theirs: FiberRef,
        known: &mut Compared,
    ) -> bool {
        if let Some(answer) = known.fibers.get(&(fiber, theirs)) {
            return *answer;
        }
        let (Some(mine), Some(yours)) = (self.get_fiber(fiber), other.get_fiber(theirs)) else {
            return false;
        };
        let alike = match (mine, yours) {
            (Fiber::Product(a), Fiber::Product(b)) => a
                .iter()
                .map(|(name, _)| name)
                .eq(b.iter().map(|(name, _)| name)),
            (Fiber::Sum(a), Fiber::Sum(b)) => {
                let head = |variants: &[(String, Option<FiberRef>)]| -> Vec<(String, bool)> {
                    let entry = |(name, payload): &(String, Option<FiberRef>)| {
                        (name.clone(), payload.is_some())
                    };
                    variants.iter().map(entry).collect()
                };
                head(a) == head(b)
            }
            (plain, theirs) => plain == theirs,
        };
        let (a, b) = (Self::parts(mine), Self::parts(yours));
        let answer = alike
            && a.len() == b.len()
            && a.iter()
                .zip(&b)
                .all(|(a, b)| self.same_fiber(*a, other, *b, known));
        known.fibers.insert((fiber, theirs), answer);
        answer
    }
}

/// The pairs already compared by `Kinds::same`.
#[derive(Default)]
struct Compared {
    kinds: HashMap<(KindRef, KindRef), bool>,
    fibers: HashMap<(FiberRef, FiberRef), bool>,
}

// Layouts: how a value sits in storage

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
    /// be checked. Each pairing of a layout with a kind or a fiber is
    /// examined once, however often it is shared.
    ///
    /// A kind whose storage is not described fits no layout.
    pub fn fits(&self, layout: LayoutRef, kinds: &Kinds, kind: KindRef) -> bool {
        self.holds(layout, kinds, Held::Kind(kind), &mut HashMap::new())
    }

    fn holds(
        &self,
        layout: LayoutRef,
        kinds: &Kinds,
        held: Held,
        known: &mut HashMap<(LayoutRef, Held), bool>,
    ) -> bool {
        if let Some(answer) = known.get(&(layout, held)) {
            return *answer;
        }
        let Some(how) = self.get(layout) else {
            return false;
        };
        let answer = match (how, held.form(kinds)) {
            (Layout::Scalar(scalar), Form::Scalar(primitive)) => scalar.stores(primitive),
            (Layout::Array(element), Form::Array(fiber)) => {
                self.holds(*element, kinds, Held::Fiber(fiber), known)
            }
            (Layout::Optional(inner), Form::Optional(kind)) => {
                self.holds(*inner, kinds, Held::Kind(kind), known)
            }
            (Layout::Group(parts), Form::Each(members)) => {
                let mut pairs = parts.iter().zip(&members);
                parts.len() == members.len()
                    && pairs.all(|(part, held)| self.holds(part.layout, kinds, *held, known))
            }
            (Layout::Group(parts), Form::Alike(member, count)) => {
                parts.len() as u64 == u64::from(count)
                    && parts
                        .iter()
                        .all(|part| self.holds(part.layout, kinds, member, known))
            }
            // A repeat stands for a group whose parts are all alike.
            (Layout::Repeat { part, count, .. }, Form::Each(members)) => {
                *count as usize == members.len()
                    && members
                        .iter()
                        .all(|held| self.holds(*part, kinds, *held, known))
            }
            (Layout::Repeat { part, count, .. }, Form::Alike(member, width)) => {
                *count == width && self.holds(*part, kinds, member, known)
            }
            _ => false,
        };
        known.insert((layout, held), answer);
        answer
    }
}

/// Something a layout may be asked to lay out.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Held {
    Kind(KindRef),
    Fiber(FiberRef),
    Field(Field),
    Scalar(Primitive),
}

/// The structure of something to be laid out, one level deep.
enum Form {
    Scalar(Primitive),
    /// Parts in order.
    Each(Vec<Held>),
    /// A stated number of parts, all alike.
    Alike(Held, u32),
    Array(FiberRef),
    Optional(KindRef),
    /// Storage that is not described.
    Unknown,
}

impl Held {
    fn form(self, kinds: &Kinds) -> Form {
        let alike = |primitive, count| Form::Alike(Held::Scalar(primitive), count);
        let fibers = |fibers: Vec<FiberRef>| fibers.into_iter().map(Held::Fiber);
        match self {
            Held::Scalar(primitive) => Form::Scalar(primitive),
            Held::Field(Field::Real) => Form::Scalar(Primitive::Real),
            Held::Field(Field::Rational) => alike(Primitive::Int, 2),
            Held::Field(Field::Complex) => alike(Primitive::Real, 2),
            Held::Fiber(fiber) => match kinds.get_fiber(fiber) {
                Some(Fiber::Bool) => Form::Scalar(Primitive::Bool),
                Some(Fiber::Int) => Form::Scalar(Primitive::Int),
                Some(Fiber::Real) => Form::Scalar(Primitive::Real),
                Some(Fiber::Rational) => alike(Primitive::Int, 2),
                Some(Fiber::Complex) => alike(Primitive::Real, 2),
                Some(Fiber::Color) => alike(Primitive::Real, 4),
                Some(Fiber::Vector { over, count }) => Form::Alike(Held::Field(*over), *count),
                Some(product @ Fiber::Product(_)) => {
                    Form::Each(fibers(Kinds::parts(product)).collect())
                }
                // An Int tag, then every variant's payload in declared order.
                Some(sum @ Fiber::Sum(_)) => {
                    let tag = std::iter::once(Held::Scalar(Primitive::Int));
                    Form::Each(tag.chain(fibers(Kinds::parts(sum))).collect())
                }
                _ => Form::Unknown,
            },
            Held::Kind(kind) => match kinds.get(kind) {
                Some(Kind::Fiber(fiber)) => Held::Fiber(*fiber).form(kinds),
                Some(Kind::Array { element, .. }) => Form::Array(*element),
                Some(Kind::Optional(inner)) => Form::Optional(*inner),
                Some(members @ (Kind::Struct(_) | Kind::Tuple(_))) => Form::Each(
                    Kinds::children(members)
                        .into_iter()
                        .map(Held::Kind)
                        .collect(),
                ),
                None => Form::Unknown,
            },
        }
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

// Units: the description of compiled code

impl ShapeValue {
    /// How many elements a length or a domain counts, or nothing if this is
    /// presence or the count does not fit in 64 bits.
    pub fn count(&self) -> Option<u64> {
        match self {
            ShapeValue::Presence(_) => None,
            ShapeValue::Length(length) => Some(*length),
            ShapeValue::Domain(axes) if axes.iter().any(|axis| axis.extent == 0) => Some(0),
            ShapeValue::Domain(axes) => axes
                .iter()
                .try_fold(1u64, |count, axis| count.checked_mul(axis.extent)),
        }
    }

    /// Whether this says an Optional value is absent. An absent value has no
    /// storage and no other shape values.
    pub fn is_absent(&self) -> bool {
        matches!(self, ShapeValue::Presence(false))
    }

    /// Whether this is a length or a domain that counts no elements. An
    /// empty value is present; an absent one is not empty.
    pub fn is_empty(&self) -> bool {
        self.count() == Some(0)
    }

    pub fn is(&self, field: ShapeField) -> bool {
        match (self, field) {
            (ShapeValue::Presence(_), ShapeField::Presence) => true,
            (ShapeValue::Length(_), ShapeField::Length) => true,
            (ShapeValue::Domain(axes), ShapeField::Domain { rank }) => axes.len() == rank,
            _ => false,
        }
    }
}

impl Checked {
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    /// The shape values that govern what a slot holds of a result. A domain
    /// governs the planes of its Array, and a presence governs everything
    /// its Optional holds. A stage that writes the slot must use each of
    /// them. Empty where the slot holds no part of a result, or a part
    /// whose shape is fixed.
    pub fn governing(&self, slot: Slot) -> &[ShapeRef] {
        self.governs.get(slot.0 as usize).map_or(&[], Vec::as_slice)
    }

    /// What produces a shape value, or nothing if the description gives it
    /// no producer. A checked unit never gives one two.
    pub fn producer(&self, shape: ShapeRef) -> Option<Producer> {
        let named = ShapeEntry::Runtime(shape);
        let brings = |input: &InputPort| input.shape.contains(&named);
        let determines = |stage: &Stage| stage.determines.contains(&shape);
        let input = self.unit.inputs.iter().position(brings);
        let stage = self.unit.stages.iter().position(determines);
        let input = input.map(|input| Producer::Input(input as u32));
        input.or(stage.map(|stage| Producer::Stage(stage as u32)))
    }

    /// Everything the description says depends on a shape value.
    pub fn consumers(&self, shape: ShapeRef) -> Vec<Consumer> {
        let unit = &self.unit;
        let named = ShapeEntry::Runtime(shape);
        let mut consumers = Vec::new();
        for (index, stage) in unit.stages.iter().enumerate() {
            if stage.uses.contains(&shape) {
                consumers.push(Consumer::Stage(index as u32));
            }
        }
        for (index, storage) in unit.storage.iter().enumerate() {
            if let Storage::Output(need) | Storage::Scratch(need) = storage {
                if need.capacity.0.contains(&named) {
                    consumers.push(Consumer::Capacity(Slot(index as u32)));
                }
            }
        }
        for (index, output) in unit.outputs.iter().enumerate() {
            if output.shape.contains(&named) {
                consumers.push(Consumer::Result(index as u32));
            }
        }
        for (index, selection) in unit.selected.iter().enumerate() {
            // An Optional's presence is the first of its shape values.
            let first = &unit.inputs[selection.first as usize];
            if selection.otherwise.is_some() && first.shape.first() == Some(&named) {
                consumers.push(Consumer::Selection(index as u32));
            }
        }
        consumers
    }
}

/// What an input port and an output port state alike.
struct Stated<'a> {
    at: PortRef,
    kind: KindRef,
    layout: LayoutRef,
    shape: &'a [ShapeEntry],
    planes: &'a [Slot],
}

/// Which shape values govern each plane of one port.
struct Governed<'a> {
    layouts: &'a Layouts,
    measures: &'a Measures,
    shape: &'a [ShapeEntry],
    /// For each plane, the facts that govern it, once something is placed
    /// there.
    over: Vec<Option<Vec<usize>>>,
    /// Planes that hold part of a value fixed as absent.
    absent_planes: Vec<bool>,
    /// Facts that belong to a part of a value fixed as absent.
    absent_facts: Vec<bool>,
}

impl Governed<'_> {
    /// Walk one layout placed at a plane, with its facts starting at `fact`.
    /// Only parts that have facts of their own are entered; everything else
    /// is one run of planes. The error is a plane two differently governed
    /// parts were placed in.
    fn walk(
        &mut self,
        layout: LayoutRef,
        plane: u32,
        fact: usize,
        enclosing: &mut Vec<usize>,
        absent: bool,
    ) -> Result<(), u32> {
        let place = layout.0 as usize;
        let planes = self.measures.planes[place];
        let facts = self.measures.facts[place] as usize;
        if absent {
            self.absent_facts[fact..fact + facts].fill(true);
        }
        match self.layouts.get(layout) {
            Some(Layout::Optional(inner)) => {
                let fixed_absent = ShapeEntry::Fixed(ShapeValue::Presence(false));
                let absent = absent || self.shape[fact] == fixed_absent;
                enclosing.push(fact);
                self.walk(*inner, plane, fact + 1, enclosing, absent)?;
                enclosing.pop();
            }
            Some(Layout::Array(_)) => {
                enclosing.push(fact);
                self.place(plane, planes, enclosing, absent)?;
                enclosing.pop();
            }
            Some(Layout::Group(parts)) if facts > 0 => {
                let mut next = fact;
                for part in parts {
                    self.walk(part.layout, plane + part.plane.0, next, enclosing, absent)?;
                    next += self.measures.facts[part.layout.0 as usize] as usize;
                }
            }
            Some(Layout::Repeat {
                part,
                count,
                plane_step,
                ..
            }) if facts > 0 => {
                let each = self.measures.facts[part.0 as usize] as usize;
                for copy in 0..*count {
                    let plane = plane + copy * plane_step;
                    self.walk(*part, plane, fact + copy as usize * each, enclosing, absent)?;
                }
            }
            _ => self.place(plane, planes, enclosing, absent)?,
        }
        Ok(())
    }

    fn place(
        &mut self,
        plane: u32,
        planes: u32,
        enclosing: &[usize],
        absent: bool,
    ) -> Result<(), u32> {
        for plane in plane..plane + planes {
            let at = plane as usize;
            match &self.over[at] {
                None => self.over[at] = Some(enclosing.to_vec()),
                Some(already) if already == enclosing => {}
                Some(_) => return Err(plane),
            }
            self.absent_planes[at] = absent;
        }
        Ok(())
    }
}

impl Unit {
    /// Check that the description is well formed and can be carried.
    pub fn check(self) -> Result<Checked, Refusal> {
        match self.well_formed() {
            Ok(Ok(checked)) => Ok(checked),
            Ok(Err(unsupported)) => Err(Refusal::Unsupported(unsupported)),
            Err(defect) => Err(Refusal::Defect(defect)),
        }
    }

    /// A defect if one is found. Otherwise the checked description, or the
    /// first port that cannot be carried.
    fn well_formed(self) -> Result<Result<Checked, Unsupported>, Defect> {
        let storage = |slot: Slot| {
            self.storage
                .get(slot.0 as usize)
                .ok_or(Defect::UnknownSlot(slot))
        };
        let exists = |entry: &ShapeEntry| match entry {
            ShapeEntry::Runtime(field) if field.0 as usize >= self.shape_fields.len() => {
                Err(Defect::UnknownShapeField(*field))
            }
            _ => Ok(()),
        };
        for (index, entry) in self.storage.iter().enumerate() {
            match entry {
                Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need) => {
                    for entry in &need.capacity.0 {
                        exists(entry)?;
                        let presence = match entry {
                            ShapeEntry::Fixed(value) => matches!(value, ShapeValue::Presence(_)),
                            ShapeEntry::Runtime(field) => {
                                self.shape_fields[field.0 as usize] == ShapeField::Presence
                            }
                        };
                        if presence {
                            return Err(Defect::CapacityNotACount(Slot(index as u32)));
                        }
                    }
                }
                Storage::Alias(of) => {
                    if !matches!(storage(*of)?, Storage::Input(_)) {
                        return Err(Defect::AliasOfNonInput(Slot(index as u32)));
                    }
                }
            }
        }

        // Each kind and each layout is stated once and well formed.
        self.kinds.check().map_err(Defect::Kind)?;
        let scalars = self.kinds.scalar_counts();
        let facts = self.kinds.shape_field_counts();
        let most_planes = u32::try_from(self.storage.len()).unwrap_or(u32::MAX);
        let measures = self.layouts.measure(most_planes).map_err(Defect::Layout)?;

        // A shape value is produced by the value bound to an input or by one
        // stage.
        let mut produced = vec![false; self.shape_fields.len()];
        // The shape values that govern what each slot holds of a result, and
        // the shape values results have.
        let mut governs: Vec<Vec<ShapeRef>> = vec![Vec::new(); self.storage.len()];
        let mut owed: Vec<ShapeRef> = Vec::new();

        let inputs = self.inputs.iter().enumerate().map(|(index, port)| Stated {
            at: PortRef::Input(index as u32),
            kind: port.kind,
            layout: port.layout,
            shape: &port.shape,
            planes: &port.planes,
        });
        let outputs = self.outputs.iter().enumerate().map(|(index, port)| Stated {
            at: PortRef::Output(index as u32),
            kind: port.kind,
            layout: port.layout,
            shape: &port.shape,
            planes: &port.planes,
        });
        for port in inputs.chain(outputs) {
            let at = port.at;
            let input = matches!(at, PortRef::Input(_));

            // The port's layout lays out its kind, and it gives one entry
            // for each fact and one slot for each plane.
            let kind = port.kind.0 as usize;
            let layout = port.layout.0 as usize;
            match scalars.get(kind) {
                None => return Err(Defect::UnknownKind(at)),
                Some(None) => return Ok(Err(Unsupported { port: at })),
                Some(Some(_)) => {}
            }
            if layout >= self.layouts.definitions.len() {
                return Err(Defect::UnknownLayout(at));
            }
            let entries = port.shape.len() as u64;
            let mismatch = Defect::KindMismatch(at);
            if facts[kind] != Some(entries)
                || measures.facts[layout] != entries
                || measures.planes[layout] as usize != port.planes.len()
                || !self.layouts.fits(port.layout, &self.kinds, port.kind)
            {
                return Err(mismatch);
            }
            for (field, entry) in self.kinds.shape_fields(port.kind).iter().zip(port.shape) {
                exists(entry)?;
                let fits = match entry {
                    ShapeEntry::Fixed(value) => value.is(*field),
                    ShapeEntry::Runtime(at) => self.shape_fields[at.0 as usize] == *field,
                };
                if !fits {
                    return Err(mismatch);
                }
            }

            // Each plane has storage of the right sort, to itself, with
            // elements large enough for what the layout keeps there.
            let mut used: Vec<Slot> = Vec::new();
            for (plane, slot) in port.planes.iter().enumerate() {
                let stated = storage(*slot)?;
                let right_sort = match stated {
                    Storage::Input(_) => input,
                    Storage::Output(_) | Storage::Alias(_) => !input,
                    Storage::Scratch(_) => false,
                };
                if !right_sort && input {
                    return Err(Defect::InputPlaneNotInput(*slot));
                } else if !right_sort {
                    return Err(Defect::OutputPlaneNotResult(*slot));
                }
                let (backing, held) = match stated {
                    Storage::Alias(of) => (*of, storage(*of)?),
                    other => (*slot, other),
                };
                let (Storage::Input(need) | Storage::Output(need) | Storage::Scratch(need)) = held
                else {
                    return Err(Defect::AliasOfNonInput(*slot));
                };
                let plane = plane as u32;
                if used.contains(&backing) {
                    return Err(Defect::PlaneRepeated {
                        port: at,
                        plane: Plane(plane),
                    });
                }
                used.push(backing);
                if measures.spans[layout][plane as usize].1 > need.element_bytes {
                    return Err(Defect::OutsideElement {
                        port: at,
                        plane: Plane(plane),
                    });
                }
            }

            // Which shape values govern each plane.
            let mut governed = Governed {
                layouts: &self.layouts,
                measures: &measures,
                shape: &port.shape,
                over: vec![None; port.planes.len()],
                absent_planes: vec![false; port.planes.len()],
                absent_facts: vec![false; port.shape.len()],
            };
            governed
                .walk(port.layout, 0, 0, &mut Vec::new(), false)
                .map_err(|plane| Defect::PlaneSharedAcrossParts {
                    port: at,
                    plane: Plane(plane),
                })?;
            let runtime = |fact: usize| match &port.shape[fact] {
                ShapeEntry::Runtime(field) => Some(*field),
                ShapeEntry::Fixed(_) => None,
            };
            if input {
                for field in (0..port.shape.len()).filter_map(runtime) {
                    produced[field.0 as usize] = true;
                }
                continue;
            }
            for (plane, slot) in port.planes.iter().enumerate() {
                if !governed.absent_planes[plane] {
                    let over = governed.over[plane].iter().flatten();
                    governs[slot.0 as usize].extend(over.filter_map(|fact| runtime(*fact)));
                }
            }
            let applies = (0..port.shape.len()).filter(|fact| !governed.absent_facts[*fact]);
            owed.extend(applies.filter_map(runtime));
        }

        // A selection chooses among inputs of the right kinds, kept one way.
        for (index, selection) in self.selected.iter().enumerate() {
            let at = PortRef::Selected(index as u32);
            let input = |index: u32| {
                self.inputs
                    .get(index as usize)
                    .ok_or(Defect::UnknownInput(at))
            };
            let first = input(selection.first)?;
            let fits = match selection.otherwise {
                None => first.kind == selection.kind,
                Some(otherwise) => {
                    let otherwise = input(otherwise)?;
                    let holds = match self.kinds.get(first.kind) {
                        Some(Kind::Optional(holds)) => Some(*holds),
                        _ => None,
                    };
                    let kept = match self.layouts.get(first.layout) {
                        Some(Layout::Optional(kept)) => Some(*kept),
                        _ => None,
                    };
                    // The other input is either what `first` may hold, or
                    // itself an Optional of it; the result is of its kind.
                    let whole = holds == Some(otherwise.kind) && kept == Some(otherwise.layout);
                    let optional = holds.is_some()
                        && first.kind == otherwise.kind
                        && first.layout == otherwise.layout;
                    otherwise.kind == selection.kind && (whole || optional)
                }
            };
            if !fits {
                return Err(Defect::KindMismatch(at));
            }
        }

        // A stage consumes only what was produced before it, and each shape
        // value has one producer.
        let mut provided = vec![false; self.storage.len()];
        for (index, stage) in self.stages.iter().enumerate() {
            let index = index as u32;
            let late = |field: ShapeRef| Defect::ShapeNotYetKnown {
                stage: index,
                field,
            };
            let place = |field: &ShapeRef| match field.0 as usize {
                place if place < self.shape_fields.len() => Ok(place),
                _ => Err(Defect::UnknownShapeField(*field)),
            };
            for field in &stage.uses {
                if !produced[place(field)?] {
                    return Err(late(*field));
                }
            }
            for slot in &stage.reads {
                storage(*slot)?;
            }
            for slot in &stage.writes {
                let (Storage::Output(need) | Storage::Scratch(need)) = storage(*slot)? else {
                    return Err(Defect::WritesShared {
                        stage: index,
                        slot: *slot,
                    });
                };
                // Providing the storage consumes what its capacity names.
                if !std::mem::replace(&mut provided[slot.0 as usize], true) {
                    for entry in &need.capacity.0 {
                        match entry {
                            ShapeEntry::Runtime(field) if !produced[field.0 as usize] => {
                                return Err(late(*field));
                            }
                            _ => {}
                        }
                    }
                }
                // Filling part of a result consumes what governs it.
                for field in &governs[slot.0 as usize] {
                    if !stage.uses.contains(field) {
                        return Err(Defect::GoverningShapeUnused {
                            stage: index,
                            field: *field,
                        });
                    }
                }
            }
            for field in &stage.determines {
                if std::mem::replace(&mut produced[place(field)?], true) {
                    return Err(Defect::ShapeProducedTwice(*field));
                }
            }
        }
        match owed.into_iter().find(|field| !produced[field.0 as usize]) {
            Some(field) => Err(Defect::ResultShapeUnknown(field)),
            None => Ok(Ok(Checked {
                unit: self,
                governs,
            })),
        }
    }
}

// Pipeline: preparation

#[cfg(feature = "pipeline")]
impl<C> Prepared<C> {
    pub fn new(unit: Checked, code: C) -> Self {
        Self { unit, code }
    }

    pub fn unit(&self) -> &Unit {
        self.unit.unit()
    }

    pub fn code(&self) -> &C {
        &self.code
    }
}
