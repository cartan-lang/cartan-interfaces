//! Implementations of the contract types: accessors and structural
//! validation.

use super::*;
use std::collections::{BTreeSet, HashMap, HashSet};

// Kinds: logical value structure

impl Field {
    /// The scalar count of one field element.
    pub fn scalars(self) -> u64 {
        match self {
            Field::Real => 1,
            Field::Rational | Field::Complex => 2,
        }
    }
}

impl Kinds {
    /// Return the fiber's reference, adding a definition only if none exists.
    pub fn fiber(&mut self, fiber: Fiber) -> FiberRef {
        let found = self.fibers.iter().position(|known| *known == fiber);
        let place = found.unwrap_or_else(|| {
            self.fibers.push(fiber);
            self.fibers.len() - 1
        });
        FiberRef(place as u32)
    }

    /// Return the kind's reference, adding a definition only if none exists.
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

    /// The fiber's immediate constituent references.
    fn parts(fiber: &Fiber) -> Vec<FiberRef> {
        match fiber {
            Fiber::Product(members) => members.iter().map(|(_, fiber)| *fiber).collect(),
            Fiber::Sum(variants) => variants.iter().filter_map(|(_, fiber)| *fiber).collect(),
            _ => Vec::new(),
        }
    }

    /// The kind's immediate constituent kind references.
    fn children(kind: &Kind) -> Vec<KindRef> {
        match kind {
            Kind::Optional(inner) => vec![*inner],
            Kind::Struct(members) => members.iter().map(|(_, kind)| *kind).collect(),
            Kind::Tuple(positions) => positions.clone(),
            Kind::String | Kind::Range(_) | Kind::Fiber(_) | Kind::Array { .. } => Vec::new(),
        }
    }

    /// Validate the tables. Each checked table contains unique definitions,
    /// so equal references denote the same represented fiber or kind.
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

    /// The scalar count of each fiber. An entry is `None` if storage is
    /// unspecified or the count exceeds 64 bits.
    ///
    /// Gap: storage for `SO2`, `SO3`, `Mat`, `Form` and `Metric` is
    /// unspecified. Ports containing these fibers are rejected as
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

    /// The scalar count per element of each kind, derived once for the table.
    /// An entry is `None` if it contains an unsupported fiber or exceeds 64
    /// bits.
    ///
    /// The tables must be validated first.
    pub fn scalar_counts(&self) -> Vec<Option<u64>> {
        let fibers = self.fiber_scalars();
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.kinds.len());
        for kind in &self.kinds {
            let count = match kind {
                Kind::Range(form) => Some(u64::from(form.endpoint_count())),
                Kind::Fiber(fiber) | Kind::Array { element: fiber, .. } => fibers[fiber.0 as usize],
                _ => Self::children(kind)
                    .iter()
                    .try_fold(0u64, |sum, kind| sum.checked_add(counts[kind.0 as usize]?)),
            };
            counts.push(count);
        }
        counts
    }

    /// The shape-field count of each kind, derived once for the table. An
    /// entry is `None` if the count exceeds 64 bits.
    pub fn shape_field_counts(&self) -> Vec<Option<u64>> {
        let mut counts: Vec<Option<u64>> = Vec::with_capacity(self.kinds.len());
        for kind in &self.kinds {
            let members = |counts: &Vec<Option<u64>>| {
                Self::children(kind)
                    .iter()
                    .try_fold(0u64, |sum, kind| sum.checked_add(counts[kind.0 as usize]?))
            };
            let count = match kind {
                Kind::Fiber(_) | Kind::Range(_) => Some(0),
                Kind::String | Kind::Array { .. } => Some(1),
                Kind::Optional(_) => members(&counts).and_then(|inner| inner.checked_add(1)),
                Kind::Struct(_) | Kind::Tuple(_) => members(&counts),
            };
            counts.push(count);
        }
        counts
    }

    /// Shape fields in port order: Optional presence before its inner fields,
    /// an Array domain, and Struct or Tuple member fields in order.
    ///
    /// Repeated kinds expand to their full field sequence. Check
    /// `shape_field_counts` before expansion. Constituents without shape
    /// fields are skipped.
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
            Some(Kind::String) => fields.push(ShapeField::EncodedLength),
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
            Some(Kind::Fiber(_) | Kind::Range(_)) | None => {}
        }
    }

    /// Compare kinds across validated tables. Each kind or fiber pair is
    /// compared once, independently of how often it is shared.
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
            (Kind::String, Kind::String) => true,
            (Kind::Range(a), Kind::Range(b)) => a == b,
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

/// Pairs already compared by `Kinds::same`.
#[derive(Default)]
struct Compared {
    kinds: HashMap<(KindRef, KindRef), bool>,
    fibers: HashMap<(FiberRef, FiberRef), bool>,
}

// Layouts: physical value representation

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

impl Kinds {
    /// A kind written as a document would write it.
    pub fn notation(&self, kind: KindRef) -> String {
        match self.get(kind) {
            Some(Kind::String) => "String".into(),
            Some(Kind::Range(form)) => format!("Range<{}>", form.notation()),
            Some(Kind::Fiber(fiber)) => self.fiber_notation(*fiber),
            Some(Kind::Array { element, axes }) => {
                let element = self.fiber_notation(*element);
                if axes.iter().all(|axis| *axis == AxisKind::Closed) {
                    format!("Array<{element}, {}>", axes.len())
                } else {
                    let axes: Vec<&str> = axes.iter().map(|axis| axis.notation()).collect();
                    format!("Array<{element}, [{}]>", axes.join(", "))
                }
            }
            Some(Kind::Optional(inner)) => format!("{}|None", self.notation(*inner)),
            Some(Kind::Struct(members)) => {
                let members: Vec<String> = members
                    .iter()
                    .map(|(name, kind)| format!("{name}: {}", self.notation(*kind)))
                    .collect();
                format!("{{{}}}", members.join(", "))
            }
            Some(Kind::Tuple(positions)) => {
                let positions: Vec<String> =
                    positions.iter().map(|kind| self.notation(*kind)).collect();
                format!("[{}]", positions.join(", "))
            }
            None => "?".to_string(),
        }
    }

    /// A fiber written as a document would write it.
    pub fn fiber_notation(&self, fiber: FiberRef) -> String {
        let field = |field: &Field| match field {
            Field::Real => "Real",
            Field::Rational => "Rational",
            Field::Complex => "Complex",
        };
        match self.get_fiber(fiber) {
            Some(Fiber::Bool) => "Bool".to_string(),
            Some(Fiber::Int) => "Int".to_string(),
            Some(Fiber::Real) => "Real".to_string(),
            Some(Fiber::Rational) => "Rational".to_string(),
            Some(Fiber::Complex) => "Complex".to_string(),
            Some(Fiber::Color) => "Color".to_string(),
            Some(Fiber::Vector { over, count }) => format!("Vector<{}, {count}>", field(over)),
            Some(Fiber::SO2) => "SO2".to_string(),
            Some(Fiber::SO3) => "SO3".to_string(),
            Some(Fiber::Mat { rows, columns }) => format!("Mat<{rows}, {columns}>"),
            Some(Fiber::Form { degree, dimension }) => format!("Form<{degree}, {dimension}>"),
            Some(Fiber::Metric { dimension }) => format!("Metric<{dimension}>"),
            Some(Fiber::Product(members)) => {
                let members: Vec<String> = members
                    .iter()
                    .map(|(name, fiber)| format!("{name}: {}", self.fiber_notation(*fiber)))
                    .collect();
                format!("({})", members.join(", "))
            }
            Some(Fiber::Sum(variants)) => {
                let variants: Vec<String> = variants
                    .iter()
                    .map(|(name, payload)| match payload {
                        Some(fiber) => format!("{name}({})", self.fiber_notation(*fiber)),
                        None => name.clone(),
                    })
                    .collect();
                variants.join(" | ")
            }
            None => "?".to_string(),
        }
    }
}

impl AxisKind {
    /// The number of finite endpoints in this Range form's payload.
    pub fn endpoint_count(self) -> u32 {
        match self {
            AxisKind::Closed | AxisKind::Periodic => 2,
            AxisKind::OpenBelow | AxisKind::OpenAbove => 1,
            AxisKind::Open => 0,
        }
    }

    /// The axis form written as a document would write it.
    pub fn notation(self) -> &'static str {
        match self {
            AxisKind::Closed => "0..n",
            AxisKind::OpenBelow => "..n",
            AxisKind::OpenAbove => "0..",
            AxisKind::Open => "..",
            AxisKind::Periodic => "..0..n",
        }
    }
}

impl Layouts {
    /// Return the layout's reference, adding a definition only if none
    /// exists.
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

    /// Compare layouts across tables. Each pair of layouts is compared once,
    /// however often it is shared.
    pub fn same(&self, layout: LayoutRef, other: &Layouts, theirs: LayoutRef) -> bool {
        self.same_layout(layout, other, theirs, &mut HashMap::new())
    }

    fn same_layout(
        &self,
        layout: LayoutRef,
        other: &Layouts,
        theirs: LayoutRef,
        known: &mut HashMap<(LayoutRef, LayoutRef), bool>,
    ) -> bool {
        if let Some(answer) = known.get(&(layout, theirs)) {
            return *answer;
        }
        let alike = match (self.get(layout), other.get(theirs)) {
            (Some(Layout::Utf8), Some(Layout::Utf8)) => true,
            (Some(Layout::Scalar(a)), Some(Layout::Scalar(b))) => a == b,
            (Some(Layout::Group(a)), Some(Layout::Group(b))) => {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(a, b)| {
                        a.plane == b.plane
                            && a.offset == b.offset
                            && self.same_layout(a.layout, other, b.layout, known)
                    })
            }
            (
                Some(Layout::Repeat {
                    part: a,
                    count: a_count,
                    plane_step: a_plane,
                    offset_step: a_offset,
                }),
                Some(Layout::Repeat {
                    part: b,
                    count: b_count,
                    plane_step: b_plane,
                    offset_step: b_offset,
                }),
            ) => {
                a_count == b_count
                    && a_plane == b_plane
                    && a_offset == b_offset
                    && self.same_layout(*a, other, *b, known)
            }
            (Some(Layout::Array(a)), Some(Layout::Array(b)))
            | (Some(Layout::Optional(a)), Some(Layout::Optional(b))) => {
                self.same_layout(*a, other, *b, known)
            }
            _ => false,
        };
        known.insert((layout, theirs), alike);
        alike
    }

    fn children(layout: &Layout) -> Vec<LayoutRef> {
        match layout {
            Layout::Utf8 | Layout::Scalar(_) => Vec::new(),
            Layout::Group(parts) => parts.iter().map(|part| part.layout).collect(),
            Layout::Repeat { part, .. } => vec![*part],
            Layout::Array(inner) | Layout::Optional(inner) => vec![*inner],
        }
    }

    /// Validate the table and derive layout properties. Each layout is
    /// limited to `most_planes`, the number of available storage regions.
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
                Layout::Utf8 => (1, vec![(0, 1)]),
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
        // Unused planes remain part of the group with empty spans.
        for span in &mut spans {
            if span.0 == u32::MAX {
                *span = (0, 0);
            }
        }
        Ok((facts, spans))
    }

    /// Test whether a layout matches a kind's structure and scalar
    /// representations. Both tables must be validated. Each layout–kind or
    /// layout–fiber pair is examined once, independently of sharing.
    ///
    /// Kinds with unspecified storage match no layout.
    pub fn fits(&self, layout: LayoutRef, kinds: &Kinds, kind: KindRef) -> bool {
        self.holds(
            layout,
            kinds,
            Representation::Kind(kind),
            &mut HashMap::new(),
        )
    }

    fn holds(
        &self,
        layout: LayoutRef,
        kinds: &Kinds,
        held: Representation,
        known: &mut HashMap<(LayoutRef, Representation), bool>,
    ) -> bool {
        if let Some(answer) = known.get(&(layout, held)) {
            return *answer;
        }
        let Some(how) = self.get(layout) else {
            return false;
        };
        let answer = match (how, held.form(kinds)) {
            (Layout::Utf8, RepresentationForm::String) => true,
            (Layout::Scalar(Scalar::I64), RepresentationForm::Endpoint) => true,
            (Layout::Scalar(scalar), RepresentationForm::Scalar(primitive)) => {
                scalar.stores(primitive)
            }
            (Layout::Array(element), RepresentationForm::Array(fiber)) => {
                self.holds(*element, kinds, Representation::Fiber(fiber), known)
            }
            (Layout::Optional(inner), RepresentationForm::Optional(kind)) => {
                self.holds(*inner, kinds, Representation::Kind(kind), known)
            }
            (Layout::Group(parts), RepresentationForm::Each(members)) => {
                let mut pairs = parts.iter().zip(&members);
                parts.len() == members.len()
                    && pairs.all(|(part, held)| self.holds(part.layout, kinds, *held, known))
            }
            (Layout::Group(parts), RepresentationForm::Alike(member, count)) => {
                parts.len() as u64 == u64::from(count)
                    && parts
                        .iter()
                        .all(|part| self.holds(part.layout, kinds, member, known))
            }
            // A repeat represents a group of identical parts.
            (Layout::Repeat { part, count, .. }, RepresentationForm::Each(members)) => {
                *count as usize == members.len()
                    && members
                        .iter()
                        .all(|held| self.holds(*part, kinds, *held, known))
            }
            (Layout::Repeat { part, count, .. }, RepresentationForm::Alike(member, width)) => {
                *count == width && self.holds(*part, kinds, member, known)
            }
            _ => false,
        };
        known.insert((layout, held), answer);
        answer
    }
}

impl Representation {
    /// The structure this representation requires of its layout, one level
    /// down. Nothing is expanded further and no stored value is read.
    pub fn form(self, kinds: &Kinds) -> RepresentationForm {
        let alike =
            |primitive, count| RepresentationForm::Alike(Representation::Scalar(primitive), count);
        let fibers = |fibers: Vec<FiberRef>| fibers.into_iter().map(Representation::Fiber);
        match self {
            Representation::Endpoint => RepresentationForm::Endpoint,
            Representation::Scalar(primitive) => RepresentationForm::Scalar(primitive),
            Representation::Field(Field::Real) => RepresentationForm::Scalar(Primitive::Real),
            Representation::Field(Field::Rational) => alike(Primitive::Int, 2),
            Representation::Field(Field::Complex) => alike(Primitive::Real, 2),
            Representation::Fiber(fiber) => match kinds.get_fiber(fiber) {
                Some(Fiber::Bool) => RepresentationForm::Scalar(Primitive::Bool),
                Some(Fiber::Int) => RepresentationForm::Scalar(Primitive::Int),
                Some(Fiber::Real) => RepresentationForm::Scalar(Primitive::Real),
                Some(Fiber::Rational) => alike(Primitive::Int, 2),
                Some(Fiber::Complex) => alike(Primitive::Real, 2),
                Some(Fiber::Color) => alike(Primitive::Real, 4),
                Some(Fiber::Vector { over, count }) => {
                    RepresentationForm::Alike(Representation::Field(*over), *count)
                }
                Some(product @ Fiber::Product(_)) => {
                    RepresentationForm::Each(fibers(Kinds::parts(product)).collect())
                }
                // An Int tag, then every variant's payload in declared order.
                Some(sum @ Fiber::Sum(_)) => {
                    let tag = std::iter::once(Representation::Scalar(Primitive::Int));
                    RepresentationForm::Each(tag.chain(fibers(Kinds::parts(sum))).collect())
                }
                _ => RepresentationForm::Unknown,
            },
            Representation::Kind(kind) => match kinds.get(kind) {
                Some(Kind::String) => RepresentationForm::String,
                Some(Kind::Range(form)) => {
                    RepresentationForm::Alike(Representation::Endpoint, form.endpoint_count())
                }
                Some(Kind::Fiber(fiber)) => Representation::Fiber(*fiber).form(kinds),
                Some(Kind::Array { element, .. }) => RepresentationForm::Array(*element),
                Some(Kind::Optional(inner)) => RepresentationForm::Optional(*inner),
                Some(members @ (Kind::Struct(_) | Kind::Tuple(_))) => RepresentationForm::Each(
                    Kinds::children(members)
                        .into_iter()
                        .map(Representation::Kind)
                        .collect(),
                ),
                None => RepresentationForm::Unknown,
            },
        }
    }
}

/// Compute spans for `count` equally spaced copies of a layout. An error
/// distinguishes overlapping copies from a representation limit.
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
        // Copies sharing planes must have non-overlapping byte spans.
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
    // Copies occupy separate planes.
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

// Units: compiled code descriptions

impl ShapeValue {
    /// The element count of a length or domain. Returns `None` for presence
    /// or a count exceeding 64 bits.
    pub fn count(&self) -> Option<u64> {
        match self {
            ShapeValue::Presence(_) => None,
            ShapeValue::Length(length) | ShapeValue::EncodedLength(length) => Some(*length),
            ShapeValue::Domain(axes) if axes.iter().any(|axis| axis.extent == 0) => Some(0),
            ShapeValue::Domain(axes) => axes
                .iter()
                .try_fold(1u64, |count, axis| count.checked_mul(axis.extent)),
        }
    }

    /// Whether this denotes Optional absence. An absent value has neither
    /// storage nor applicable inner shape values.
    pub fn is_absent(&self) -> bool {
        matches!(self, ShapeValue::Presence(false))
    }

    /// Whether a length or domain contains zero elements. Emptiness denotes a
    /// present value, distinct from absence.
    pub fn is_empty(&self) -> bool {
        self.count() == Some(0)
    }

    pub fn is(&self, field: ShapeField) -> bool {
        match (self, field) {
            (ShapeValue::Presence(_), ShapeField::Presence) => true,
            (ShapeValue::Length(_), ShapeField::Length) => true,
            (ShapeValue::EncodedLength(_), ShapeField::EncodedLength) => true,
            (ShapeValue::Domain(axes), ShapeField::Domain { rank }) => axes.len() == rank,
            _ => false,
        }
    }
}

impl Checked {
    pub fn unit(&self) -> &Unit {
        &self.unit
    }

    /// Shape dependencies governing the result portion stored in a slot.
    /// Array domains govern their planes; Optional presence governs its
    /// contents. Writers must consume all these dependencies. Returns an
    /// empty slice for non-result storage or fixed shape.
    pub fn governing(&self, slot: Slot) -> &[ShapeRef] {
        self.governs.get(slot.0 as usize).map_or(&[], Vec::as_slice)
    }

    /// The declared producer, or `None` if absent. A checked unit has at most
    /// one producer per shape value.
    pub fn producer(&self, shape: ShapeRef) -> Option<Producer> {
        let named = ShapeEntry::Runtime(shape);
        let brings = |input: &InputPort| input.shape.contains(&named);
        let determines = |stage: &Stage| stage.determines.contains(&shape);
        let input = self.unit.inputs.iter().position(brings);
        let stage = self.unit.stages.iter().position(determines);
        let input = input.map(|input| Producer::Input(input as u32));
        input.or(stage.map(|stage| Producer::Stage(stage as u32)))
    }

    /// All declared consumers of a shape value.
    pub fn consumers(&self, shape: ShapeRef) -> Vec<Consumer> {
        let unit = &self.unit;
        let named = ShapeEntry::Runtime(shape);
        let mut consumers = Vec::new();
        for (index, stage) in unit.stages.iter().enumerate() {
            if stage.uses.contains(&shape) {
                consumers.push(Consumer::Stage(index as u32));
            }
            if stage.extent == Some(named.clone()) {
                consumers.push(Consumer::Extent(index as u32));
            }
        }
        for (index, input) in unit.inputs.iter().enumerate() {
            if input.assumes.iter().any(|assumed| assumed.equals == shape) {
                consumers.push(Consumer::Input(index as u32));
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

/// Description fields shared by input and output ports.
struct Stated<'a> {
    at: PortRef,
    kind: KindRef,
    layout: LayoutRef,
    shape: &'a [ShapeEntry],
    planes: &'a [Slot],
}

/// Shape dependencies governing each plane of a port.
struct Governed<'a> {
    layouts: &'a Layouts,
    measures: &'a Measures,
    shape: &'a [ShapeEntry],
    /// The shape fields governing each occupied plane.
    over: Vec<Option<Vec<usize>>>,
    /// Planes that hold part of a value fixed as absent.
    absent_planes: Vec<bool>,
    /// Shape fields belonging to a part fixed as absent.
    absent_facts: Vec<bool>,
}

impl Governed<'_> {
    /// Traverse a layout at the specified plane and shape-field index `fact`.
    /// Only constituents with shape fields are traversed; other constituents
    /// form a contiguous plane range. An error identifies a plane shared by
    /// parts with different shape dependencies.
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
            Some(Layout::Array(_) | Layout::Utf8) => {
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

impl ShapeField {
    fn checked_words(self) -> Option<u32> {
        match self {
            ShapeField::Presence | ShapeField::Length | ShapeField::EncodedLength => Some(1),
            ShapeField::Domain { rank } => u32::try_from(rank).ok()?.checked_mul(2),
        }
    }

    /// The word count for a representable shape-field category in a [`Frame`].
    pub fn words(self) -> u32 {
        self.checked_words().expect("shape fits a frame")
    }
}

impl Stage {
    /// Derive the launch frame from the unit's shape-field table. The
    /// referenced shapes and the frame's word count must be valid, as
    /// guaranteed by a checked unit.
    pub fn frame(&self, shape_fields: &[ShapeField]) -> Frame {
        self.checked_frame(shape_fields)
            .expect("checked stage frame")
    }

    fn checked_frame(&self, shape_fields: &[ShapeField]) -> Option<Frame> {
        let prefix = self.reads.len().checked_add(self.writes.len())?;
        let mut words = u32::try_from(prefix).ok()?;
        let slots = self.reads.iter().chain(&self.writes).copied().collect();
        let mut shapes = Vec::new();
        for shape in self.uses.iter().chain(&self.determines) {
            shapes.push((*shape, words));
            let field = shape_fields.get(shape.0 as usize)?;
            words = words.checked_add(field.checked_words()?)?;
        }
        let mut suffix = None;
        if let Some(window) = &self.window {
            let first = words;
            words = words.checked_add(2)?;
            let mut starts = Vec::new();
            let mut seen = HashSet::new();
            for &stencil in window.reads.iter().flatten() {
                if seen.insert(stencil) {
                    starts.push((stencil, words));
                    words = words.checked_add(1)?;
                }
            }
            suffix = Some(WindowFrame {
                first,
                count: first + 1,
                starts,
            });
        }
        Some(Frame {
            slots,
            shapes,
            window: suffix,
            words,
        })
    }
}

impl Capacity {
    /// Prove equality to one domain count times a fixed multiplier. A
    /// dynamic factor must be the very same domain, exactly once.
    fn matches_sites(&self, domain: &ShapeEntry, elements: u32) -> bool {
        let mut dynamic = None;
        let mut product = 1u64;
        for factor in &self.0 {
            match factor {
                ShapeEntry::Runtime(field) => {
                    if dynamic.replace(*field).is_some() {
                        return false;
                    }
                }
                ShapeEntry::Fixed(value) => {
                    let Some(next) = value.count().and_then(|count| product.checked_mul(count))
                    else {
                        return false;
                    };
                    product = next;
                }
            }
        }
        let elements = u64::from(elements);
        match domain {
            ShapeEntry::Runtime(field) => dynamic == Some(*field) && product == elements,
            ShapeEntry::Fixed(value) => {
                dynamic.is_none()
                    && value.count().and_then(|sites| sites.checked_mul(elements)) == Some(product)
            }
        }
    }
}

impl Unit {
    /// The global domain of a checked rank-one Array port, optionally
    /// enclosed in an Optional. Other structures cannot be stencil inputs.
    fn window_domain<'a>(&self, kind: KindRef, shape: &'a [ShapeEntry]) -> Option<&'a ShapeEntry> {
        let (kind, fields) = match self.kinds.get(kind)? {
            Kind::Optional(inner) => (self.kinds.get(*inner)?, shape.get(1..)?),
            other => (other, shape),
        };
        match (kind, fields) {
            (Kind::Array { axes, .. }, [domain])
                if matches!(axes.as_slice(), [AxisKind::Closed | AxisKind::Periodic]) =>
            {
                Some(domain)
            }
            _ => None,
        }
    }

    fn check_stencil_read(
        &self,
        index: u32,
        stage: &Stage,
        slot: Slot,
        stencil: StencilRef,
    ) -> Result<(), Defect> {
        let mismatch = Defect::StencilRead {
            stage: index,
            slot,
            stencil,
        };
        let descriptor = self.stencils.get(stencil.0 as usize).ok_or(mismatch)?;
        let port = &self.inputs[descriptor.input as usize];
        let backing = match self.storage[slot.0 as usize] {
            Storage::Alias(input) => input,
            _ => slot,
        };
        if !port.planes.contains(&backing) {
            return Err(mismatch);
        }
        let Storage::Input(need) = &self.storage[backing.0 as usize] else {
            return Err(mismatch);
        };
        let domain = self.window_domain(port.kind, &port.shape).ok_or(mismatch)?;
        if !need.capacity.matches_sites(domain, 1) {
            return Err(Defect::WindowCapacity { stage: index, slot });
        }
        // The entry can consume the original field or a declared
        // equality verified before reading the backing input.
        for (at, entry) in port.shape.iter().enumerate() {
            let ShapeEntry::Runtime(field) = entry else {
                continue;
            };
            let assumed = port.assumes.iter().any(|assumed| {
                assumed.field as usize == at && stage.uses.contains(&assumed.equals)
            });
            if !stage.uses.contains(field) && !assumed {
                return Err(Defect::GoverningShapeUnused {
                    stage: index,
                    field: *field,
                });
            }
        }
        Ok(())
    }

    fn check_window(
        &self,
        index: u32,
        stage: &Stage,
        window: &RankOneWindow,
    ) -> Result<(), Defect> {
        if stage.reads.len() != window.reads.len() || stage.writes.len() != window.writes.len() {
            return Err(Defect::WindowArity(index));
        }
        let extent = stage.extent.as_ref().ok_or(Defect::WindowExtent(index))?;
        let rank_one = match extent {
            ShapeEntry::Fixed(value) => value.is(ShapeField::Domain { rank: 1 }),
            ShapeEntry::Runtime(field) => {
                self.shape_fields[field.0 as usize] == ShapeField::Domain { rank: 1 }
            }
        };
        if !rank_one || !stage.determines.is_empty() {
            return Err(Defect::WindowExtent(index));
        }
        for (&slot, &stencil) in stage.reads.iter().zip(&window.reads) {
            if let Some(stencil) = stencil {
                self.check_stencil_read(index, stage, slot, stencil)?;
            }
        }
        let mut seen = HashMap::new();
        for (&slot, &ownership) in stage.writes.iter().zip(&window.writes) {
            if let Some(previous) = seen.insert(slot.0, ownership) {
                if previous != ownership {
                    return Err(Defect::WindowWriteConflict { stage: index, slot });
                }
                continue;
            }
            let WriteOwnership::BySite { elements } = ownership else {
                continue;
            };
            let (Storage::Output(need) | Storage::Scratch(need)) = &self.storage[slot.0 as usize]
            else {
                return Err(Defect::WritesShared { stage: index, slot });
            };
            let defect = Defect::WindowCapacity { stage: index, slot };
            if !need.capacity.matches_sites(extent, elements.get()) {
                return Err(defect);
            }
            for port in self
                .outputs
                .iter()
                .filter(|port| port.planes.contains(&slot))
            {
                let domain = self.window_domain(port.kind, &port.shape).ok_or(defect)?;
                if elements.get() != 1 || !need.capacity.matches_sites(domain, 1) {
                    return Err(defect);
                }
            }
        }
        Ok(())
    }

    /// Validate the description's structure and supported representations.
    pub fn check(self) -> Result<Checked, Refusal> {
        match self.well_formed() {
            Ok(Ok(checked)) => Ok(checked),
            Ok(Err(unsupported)) => Err(Refusal::Unsupported(unsupported)),
            Err(defect) => Err(Refusal::Defect(defect)),
        }
    }

    /// Return a detected defect, the validated description, or the first
    /// unsupported port.
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

        // Validate unique kind and layout definitions.
        self.kinds.check().map_err(Defect::Kind)?;
        let scalars = self.kinds.scalar_counts();
        let facts = self.kinds.shape_field_counts();
        // A table may hold layouts this unit does not use, so no layout is
        // limited by the unit's storage; each port's planes are counted below.
        let measures = self.layouts.measure(u32::MAX).map_err(Defect::Layout)?;

        // Shape values originate from bound inputs or stages.
        let mut produced = vec![false; self.shape_fields.len()];
        // Track slot shape dependencies and result shape fields.
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

            // Check layout compatibility, shape-field count and plane count.
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

            // Check each plane's storage role, exclusivity and element size.
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

            // Collect each plane's shape dependencies.
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

        for (index, stencil) in self.stencils.iter().enumerate() {
            let defect = Defect::InvalidStencil(StencilRef(index as u32));
            let port = self.inputs.get(stencil.input as usize).ok_or(defect)?;
            if stencil.min_offset > stencil.max_offset
                || stencil
                    .valid_coordinates
                    .is_some_and(|bounds| bounds.min > bounds.max)
                || self.window_domain(port.kind, &port.shape).is_none()
            {
                return Err(defect);
            }
        }

        // Selections require compatible input kinds and identical layouts.
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
                    // The fallback may be the inner kind or its Optional
                    // form; its kind determines the result.
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

        // What an input assumes of its shape is a value some stage
        // determines, of the sort of the field it is assumed for.
        let mut assumes: Vec<Vec<ShapeRef>> = vec![Vec::new(); self.storage.len()];
        for (index, port) in self.inputs.iter().enumerate() {
            let stated = self.kinds.shape_fields(port.kind);
            for assumed in &port.assumes {
                let field = stated.get(assumed.field as usize);
                let named = self.shape_fields.get(assumed.equals.0 as usize);
                let determined = self
                    .stages
                    .iter()
                    .any(|stage| stage.determines.contains(&assumed.equals));
                if field.is_none() || field != named || !determined {
                    return Err(Defect::AssumedShapeUnknown {
                        input: index as u32,
                        equals: assumed.equals,
                    });
                }
                for slot in &port.planes {
                    assumes[slot.0 as usize].push(assumed.equals);
                }
            }
        }

        // Enforce production before consumption and unique shape producers.
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
            // The runtime holds a stage's extent before it launches it.
            match &stage.extent {
                None => {}
                Some(ShapeEntry::Fixed(ShapeValue::Presence(_) | ShapeValue::EncodedLength(_))) => {
                    return Err(Defect::ExtentNotACount(index));
                }
                Some(ShapeEntry::Fixed(_)) => {}
                Some(ShapeEntry::Runtime(field)) => {
                    let at = place(field)?;
                    if matches!(
                        self.shape_fields[at],
                        ShapeField::Presence | ShapeField::EncodedLength
                    ) {
                        return Err(Defect::ExtentNotACount(index));
                    }
                    if !produced[at] {
                        return Err(late(*field));
                    }
                    if !stage.uses.contains(field) {
                        return Err(Defect::ExtentUnused(index));
                    }
                }
            }
            if stage.extent.is_some() {
                if let Some(slot) = stage.writes.iter().find(|slot| stage.reads.contains(slot)) {
                    return Err(Defect::ReadsWhatItWrites {
                        stage: index,
                        slot: *slot,
                    });
                }
            }
            for slot in &stage.reads {
                // An alias reads the same input storage and requires the
                // same assumed shape to have been determined.
                let backing = match storage(*slot)? {
                    Storage::Alias(input) => input,
                    _ => slot,
                };
                if let Some(field) = assumes[backing.0 as usize]
                    .iter()
                    .find(|field| !produced[field.0 as usize])
                {
                    return Err(late(*field));
                }
            }
            for slot in &stage.writes {
                let (Storage::Output(need) | Storage::Scratch(need)) = storage(*slot)? else {
                    return Err(Defect::WritesShared {
                        stage: index,
                        slot: *slot,
                    });
                };
                // Storage provision consumes capacity dependencies.
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
                // Result writes consume the governing shape values.
                for field in &governs[slot.0 as usize] {
                    if !stage.uses.contains(field) {
                        return Err(Defect::GoverningShapeUnused {
                            stage: index,
                            field: *field,
                        });
                    }
                }
            }
            if let Some(window) = &stage.window {
                self.check_window(index, stage, window)?;
            }
            for field in &stage.determines {
                if std::mem::replace(&mut produced[place(field)?], true) {
                    return Err(Defect::ShapeProducedTwice(*field));
                }
            }
            stage
                .checked_frame(&self.shape_fields)
                .ok_or(Defect::FrameTooLarge(index))?;
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

impl HostCode {
    /// Check native operations and their exact correspondence to a checked unit.
    /// This checks descriptions, not the results of executing a formatter.
    pub fn check(&self, checked: &Checked) -> Result<(), HostDefect> {
        let unit = checked.unit();
        if !unit.inputs.is_empty() || !unit.selected.is_empty() {
            return Err(HostDefect::Inputs);
        }
        for (index, operation) in self.text.iter().enumerate() {
            let at = TextRef(index as u32);
            match operation {
                TextOp::Literal(_) => {}
                TextOp::Concat(parts) => {
                    for names in parts {
                        if names.0 as usize >= index {
                            return Err(HostDefect::TextReference { at, names: *names });
                        }
                    }
                }
                TextOp::Format { value, format } => {
                    if format.digits.is_some_and(|digits| digits > 17)
                        || matches!(value, TextScalar::Bool(_)) && *format != TextFormat::default()
                    {
                        return Err(HostDefect::Format { at });
                    }
                }
            }
        }
        for (index, storage) in unit.storage.iter().enumerate() {
            let slot = Slot(index as u32);
            let Storage::Output(need) = storage else {
                return Err(HostDefect::Storage { slot });
            };
            if need.memory != Memory::Host || need.element_bytes != 1 || need.align != 1 {
                return Err(HostDefect::Storage { slot });
            }
        }
        let mut demands = BTreeSet::new();
        for (index, port) in unit.outputs.iter().enumerate() {
            if !demands.insert(port.demand)
                || !matches!(port.demand, Demand::Value(Node::Derived(_)))
                || !matches!(unit.kinds.get(port.kind), Some(Kind::String))
                || !matches!(unit.layouts.get(port.layout), Some(Layout::Utf8))
                || port.planes.len() != 1
                || port.shape.len() != 1
            {
                return Err(HostDefect::Output {
                    index: index as u32,
                });
            }
            let Some(Storage::Output(need)) = unit.storage.get(port.planes[0].0 as usize) else {
                return Err(HostDefect::Output {
                    index: index as u32,
                });
            };
            if need.capacity.0 != port.shape {
                return Err(HostDefect::Output {
                    index: index as u32,
                });
            }
        }
        if self.stages.len() != unit.stages.len() {
            return Err(HostDefect::StageCount);
        }
        let mut measurements = HashMap::new();
        let mut filled = HashSet::new();
        for (index, (operation, stage)) in self.stages.iter().zip(&unit.stages).enumerate() {
            let defect = HostDefect::Stage {
                index: index as u32,
            };
            if !stage.reads.is_empty() || stage.extent.is_some() {
                return Err(defect);
            }
            let (text, valid) = match operation {
                HostStage::Measure { text, output } => {
                    let valid = unit.shape_fields.get(output.0 as usize)
                        == Some(&ShapeField::EncodedLength)
                        && stage.writes.is_empty()
                        && stage.uses.is_empty()
                        && stage.determines == [*output];
                    measurements.insert(output.0, *text);
                    (*text, valid)
                }
                HostStage::Fill {
                    text,
                    output,
                    length,
                } => {
                    let uses = match length {
                        ShapeEntry::Fixed(ShapeValue::EncodedLength(length)) if matches!(self.text.get(text.0 as usize), Some(TextOp::Literal(value)) if value.len() as u64 == *length) => {
                            Vec::new()
                        }
                        ShapeEntry::Runtime(reference)
                            if measurements.get(&reference.0) == Some(text) =>
                        {
                            vec![*reference]
                        }
                        _ => return Err(defect),
                    };
                    let Some(Storage::Output(need)) = unit.storage.get(output.0 as usize) else {
                        return Err(defect);
                    };
                    let result = unit
                        .outputs
                        .iter()
                        .any(|port| port.planes == [*output] && port.shape == [length.clone()]);
                    let valid = stage.writes == [*output]
                        && stage.uses == uses
                        && stage.determines.is_empty()
                        && need.capacity.0 == [length.clone()]
                        && result
                        && filled.insert(output.0);
                    (*text, valid)
                }
            };
            if !valid || text.0 as usize >= self.text.len() {
                return Err(defect);
            }
        }
        for (index, _) in unit.storage.iter().enumerate() {
            if !filled.contains(&(index as u32)) {
                return Err(HostDefect::Storage {
                    slot: Slot(index as u32),
                });
            }
        }
        Ok(())
    }
}
