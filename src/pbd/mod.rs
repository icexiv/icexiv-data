//! Win32 PBD race-deformer matrices, connection forests and EPBD extensions.
//!
//! Parsing borrows the input, validates all references, forest consistency,
//! names, matrix spans and embedded PBDs under shared work/record budgets.
//! Matrices are the actual file coefficients, not a rest-pose approximation.
//! Names remain bytes; no I/O, race fallback, deformation baking or skeleton
//! matching is performed. EPBD condition integers and optional root scalars
//! remain raw; no native selection/default policy is applied.
mod flat;
use crate::binary::{record, slice, u16_at, u32_at};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidData(&'static str),
    BudgetExceeded(&'static str),
    UnsupportedPack,
    IndexOutOfBounds,
}
impl From<&'static str> for Error {
    fn from(s: &'static str) -> Self {
        Self::InvalidData(s)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData(s) => write!(f, "invalid PBD: {s}"),
            Self::BudgetExceeded(s) => write!(f, "PBD {s} budget exceeded"),
            Self::UnsupportedPack => {
                f.write_str("unsupported PBD PACK extension type, version or count")
            }
            Self::IndexOutOfBounds => f.write_str("PBD index out of bounds"),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_deformers: usize,
    pub max_bones: usize,
    pub max_validation_steps: usize,
    pub max_name_bytes: usize,
    pub max_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_deformers: 4096,
            max_bones: 1_000_000,
            max_validation_steps: 16 * 1024 * 1024,
            max_name_bytes: 4096,
            max_depth: 8,
        }
    }
}
struct Budget {
    deformers: usize,
    bones: usize,
    steps: usize,
    name: usize,
    depth: usize,
}
fn spend(value: &mut usize, count: usize, name: &'static str) -> Result<()> {
    *value = value
        .checked_sub(count)
        .ok_or(Error::BudgetExceeded(name))?;
    Ok(())
}

/// Three row-major affine rows, each with translation in its fourth lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix(pub [[f32; 4]; 3]);
/// Failure of a fixed-size affine inverse, independent of file parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatrixInverseError {
    NonFiniteInput,
    Singular,
    OutputOutOfRange,
}
impl fmt::Display for MatrixInverseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NonFiniteInput => "nonfinite affine matrix input",
            Self::Singular => "singular affine matrix",
            Self::OutputOutOfRange => "affine inverse exceeds finite f32 range",
        })
    }
}
impl std::error::Error for MatrixInverseError {}
impl Matrix {
    /// Invert an affine matrix using f64 partial-pivot Gauss–Jordan elimination.
    ///
    /// Uses constant stack storage and retains shear; no TRS approximation,
    /// regularization or epsilon-based singularity threshold is applied. A zero
    /// computed pivot is singular. Ill-conditioned inputs can amplify rounding;
    /// callers requiring a condition-number policy must apply their own checks.
    /// Nonfinite inputs and inverses outside finite f32 range are explicit errors.
    /// This does not interpret a reverse race pair, bind space or PBD route.
    pub fn try_inverse(self) -> std::result::Result<Self, MatrixInverseError> {
        if !self.0.iter().flatten().all(|v| v.is_finite()) {
            return Err(MatrixInverseError::NonFiniteInput);
        }
        let mut augmented = [[0f64; 6]; 3];
        for (r, row) in augmented.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().take(3).enumerate() {
                *value = f64::from(self.0[r][c]);
            }
            row[r + 3] = 1.;
        }
        for col in 0..3 {
            let mut pivot = col;
            for row in col + 1..3 {
                if augmented[row][col].abs() > augmented[pivot][col].abs() {
                    pivot = row;
                }
            }
            if augmented[pivot][col] == 0. {
                return Err(MatrixInverseError::Singular);
            }
            augmented.swap(col, pivot);
            let divisor = augmented[col][col];
            for value in &mut augmented[col] {
                *value /= divisor;
            }
            let normalized = augmented[col];
            for (row, values) in augmented.iter_mut().enumerate() {
                if row != col {
                    let factor = values[col];
                    for (value, pivot_value) in values.iter_mut().zip(normalized) {
                        *value -= factor * pivot_value;
                    }
                }
            }
        }
        let mut result = [[0f32; 4]; 3];
        for (r, row) in result.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().take(3).enumerate() {
                *value = augmented[r][c + 3] as f32;
            }
            row[3] = -(0..3)
                .map(|c| augmented[r][c + 3] * f64::from(self.0[c][3]))
                .sum::<f64>() as f32;
        }
        if !result.iter().flatten().all(|v| v.is_finite()) {
            return Err(MatrixInverseError::OutputOutOfRange);
        }
        Ok(Self(result))
    }
    /// Affine composition `self * rhs` for column vectors: `rhs` acts first.
    /// This is ordinary matrix multiplication, not PBD route selection or
    /// character assembly. It preserves affine shear without TRS decomposition.
    pub fn compose(self, rhs: Self) -> Self {
        let mut rows = [[0.; 4]; 3];
        for (r, row) in rows.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().enumerate() {
                *value = (0..3).map(|k| self.0[r][k] * rhs.0[k][c]).sum();
                if c == 3 {
                    *value += self.0[r][3];
                }
            }
        }
        Self(rows)
    }
    pub fn column_major_4x4(self) -> [f32; 16] {
        let r = self.0;
        [
            r[0][0], r[1][0], r[2][0], 0., r[0][1], r[1][1], r[2][1], 0., r[0][2], r[1][2],
            r[2][2], 0., r[0][3], r[1][3], r[2][3], 1.,
        ]
    }
    pub fn transform_point(self, p: [f32; 3]) -> [f32; 3] {
        self.0
            .map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3])
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Bone<'a> {
    pub name: &'a [u8],
    pub matrix: Matrix,
}

#[derive(Clone, Copy, Debug)]
pub struct Deformer<'a> {
    pub skeleton_id: u16,
    pub connection_index: usize,
    /// Record-local scalar at byte offset 8. A Win64 consumer queries two
    /// skeleton IDs and stores their scalar ratio separately from bone matrices.
    /// The final parameter meaning and deformation space are not established;
    /// this value is never automatically applied to the affine coefficients.
    pub unknown_scale: f32,
    data: &'a [u8],
    names: &'a [u8],
    matrices: &'a [u8],
}
impl<'a> Deformer<'a> {
    pub fn has_payload(self) -> bool {
        !self.data.is_empty()
    }
    pub fn bone_count(self) -> usize {
        self.names.len() / 2
    }
    pub fn bone(self, index: usize) -> Result<Bone<'a>> {
        if index >= self.bone_count() {
            return Err(Error::IndexOutOfBounds);
        }
        let start = usize::from(u16_at(self.names, index * 2)?);
        let data = self.data.get(start..).ok_or("bone name offset")?;
        let end = data
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated bone name")?;
        let raw = record(self.matrices, index, 48)?;
        let mut rows = [[0.; 4]; 3];
        for (row, target) in rows.iter_mut().enumerate() {
            for (col, v) in target.iter_mut().enumerate() {
                *v = f32::from_bits(u32_at(raw, (row * 4 + col) * 4)?);
            }
        }
        Ok(Bone {
            name: &data[..end],
            matrix: Matrix(rows),
        })
    }
    pub fn find_bone(self, name: &[u8]) -> Result<Option<Bone<'a>>> {
        for i in 0..self.bone_count() {
            let bone = self.bone(i)?;
            if bone.name == name {
                return Ok(Some(bone));
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Connection {
    pub parent: Option<usize>,
    pub first_child: Option<usize>,
    pub next_sibling: Option<usize>,
    pub deformer_index: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Pbd<'a> {
    data: &'a [u8],
    headers: &'a [u8],
    connections: &'a [u8],
    core_end: usize,
    extension: Option<Extended<'a>>,
}

/// File-local deformer table indices, from descendant towards (but excluding)
/// the ancestor. Construct with [`Pbd::ancestor_path`]. Iteration borrows the
/// validated forest and does not allocate or choose deformation policy.
#[derive(Debug)]
pub struct AncestorPath<'a> {
    pbd: Pbd<'a>,
    cursor: Option<usize>,
    remaining: usize,
}
impl Iterator for AncestorPath<'_> {
    type Item = Result<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let result = self
            .cursor
            .ok_or(Error::InvalidData("ancestry ended early"))
            .and_then(|i| self.pbd.connection(i));
        match result {
            Ok(c) => {
                self.cursor = c.parent;
                Some(Ok(c.deformer_index))
            }
            Err(e) => {
                self.remaining = 0;
                self.cursor = None;
                Some(Err(e))
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.remaining))
    }
}
impl std::iter::FusedIterator for AncestorPath<'_> {}

impl<'a> Pbd<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let mut budget = Budget {
            deformers: limits.max_deformers,
            bones: limits.max_bones,
            steps: limits.max_validation_steps,
            name: limits.max_name_bytes,
            depth: limits.max_depth,
        };
        Self::parse_inner(bytes, &mut budget)
    }
    fn skeleton(bytes: &'a [u8]) -> Result<Self> {
        let (core_end, extension) = pack(bytes)?;
        let data = &bytes[..core_end];
        let count = usize::try_from(u32_at(data, 0)?)
            .map_err(|_| Error::InvalidData("deformer count overflow"))?;
        if count > 32768 {
            return Err(Error::InvalidData(
                "connection count exceeds signed indices",
            ));
        }
        let headers = slice(data, 4, count * 12)?;
        let connections = slice(data, 4 + count * 12, count * 8)?;
        Ok(Self {
            data,
            headers,
            connections,
            core_end,
            extension,
        })
    }
    fn parse_inner(bytes: &'a [u8], budget: &mut Budget) -> Result<Self> {
        spend(&mut budget.depth, 1, "nesting")?;
        let pbd = Self::skeleton(bytes)?;
        let n = pbd.deformer_count();
        let mut used_end = 4 + pbd.headers.len() + pbd.connections.len();
        spend(&mut budget.deformers, n, "deformers")?;
        for i in 0..n {
            spend(&mut budget.steps, n + 1, "validation")?;
            let d = pbd.deformer(i)?;
            if d.has_payload() {
                used_end = used_end.max(
                    d.matrices.as_ptr() as usize - pbd.data.as_ptr() as usize + d.matrices.len(),
                );
            }
            if pbd.connection(d.connection_index)?.deformer_index != i {
                return Err(Error::InvalidData("deformer/connection inverse mismatch"));
            }
            for j in 0..i {
                if pbd.deformer_id(j)? == d.skeleton_id {
                    return Err(Error::InvalidData("duplicate skeleton ID"));
                }
            }
            spend(&mut budget.bones, d.bone_count(), "bones")?;
            for bone in 0..d.bone_count() {
                let start = usize::from(u16_at(d.names, bone * 2)?);
                let matrix_end =
                    d.matrices.as_ptr() as usize - d.data.as_ptr() as usize + d.matrices.len();
                if start < matrix_end {
                    return Err(Error::InvalidData("bone name overlaps tables"));
                }
                let name = d.data.get(start..).ok_or("bone name offset")?;
                let bound = name
                    .len()
                    .min(budget.name.saturating_add(1))
                    .min(budget.steps);
                spend(&mut budget.steps, bound, "name validation")?;
                let end = name[..bound]
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or(Error::InvalidData("unterminated or over-budget bone name"))?;
                used_end =
                    used_end.max(name.as_ptr() as usize - pbd.data.as_ptr() as usize + end + 1);
            }
        }
        let tail = pbd.data.get(used_end..).ok_or("payload used end")?;
        spend(&mut budget.steps, tail.len(), "trailing padding")?;
        if tail.iter().any(|&b| b != 0) {
            return Err(Error::InvalidData("nonzero unrecognized PBD tail"));
        }
        for i in 0..n {
            let c = pbd.connection(i)?;
            if pbd.deformer(c.deformer_index)?.connection_index != i {
                return Err(Error::InvalidData("connection/deformer inverse mismatch"));
            }
            let mut at = Some(i);
            for _ in 0..=n {
                spend(&mut budget.steps, 1, "parent validation")?;
                let Some(node) = at else {
                    break;
                };
                at = pbd.connection(node)?.parent;
            }
            if at.is_some() {
                return Err(Error::InvalidData("parent cycle"));
            }
            if let Some(sibling) = c.next_sibling {
                if pbd.connection(sibling)?.parent != c.parent {
                    return Err(Error::InvalidData("sibling parent mismatch"));
                }
            }
            let mut sibling = c.next_sibling;
            for _ in 0..n {
                spend(&mut budget.steps, 1, "sibling validation")?;
                let Some(node) = sibling else {
                    break;
                };
                sibling = pbd.connection(node)?.next_sibling;
            }
            if sibling.is_some() {
                return Err(Error::InvalidData("sibling cycle"));
            }
            let mut child = c.first_child;
            for _ in 0..=n {
                spend(&mut budget.steps, 1, "child validation")?;
                let Some(node) = child else {
                    break;
                };
                let v = pbd.connection(node)?;
                if v.parent != Some(i) {
                    return Err(Error::InvalidData("child parent mismatch"));
                }
                child = v.next_sibling;
            }
            if child.is_some() {
                return Err(Error::InvalidData("sibling cycle"));
            }
            // Every non-root appears in its parent's child chain.
            if let Some(parent) = c.parent {
                let mut at = pbd.connection(parent)?.first_child;
                let mut found = false;
                for _ in 0..n {
                    spend(&mut budget.steps, 1, "child membership")?;
                    let Some(node) = at else {
                        break;
                    };
                    if node == i {
                        found = true;
                        break;
                    }
                    at = pbd.connection(node)?.next_sibling;
                }
                if !found {
                    return Err(Error::InvalidData("unlinked child"));
                }
            }
        }
        if let Some(ext) = pbd.extension {
            spend(&mut budget.steps, ext.entry_count(), "extension entries")?;
            for i in 0..ext.entry_count() {
                let e = ext.entry(i)?;
                spend(
                    &mut budget.steps,
                    e.raw_conditions().len() / 4,
                    "extension conditions",
                )?;
                Self::parse_inner(e.pbd_bytes(), budget)?;
            }
        }
        budget.depth += 1;
        Ok(pbd)
    }
    pub fn deformer_count(self) -> usize {
        self.headers.len() / 12
    }
    pub fn deformer_id(self, index: usize) -> Result<u16> {
        Ok(u16_at(record(self.headers, index, 12)?, 0)?)
    }
    pub fn deformer(self, index: usize) -> Result<Deformer<'a>> {
        let h = record(self.headers, index, 12)?;
        let skeleton_id = u16_at(h, 0)?;
        let connection_index = usize::from(u16_at(h, 2)?);
        let unknown_scale = f32::from_bits(u32_at(h, 8)?);
        let start = usize::try_from(u32_at(h, 4)?)
            .map_err(|_| Error::InvalidData("payload offset overflow"))?;
        let mut d = Deformer {
            skeleton_id,
            connection_index,
            unknown_scale,
            data: &[],
            names: &[],
            matrices: &[],
        };
        if start == 0 {
            return Ok(d);
        }
        if start < 4 + self.headers.len() + self.connections.len() || start % 4 != 0 {
            return Err(Error::InvalidData(
                "payload overlaps header or is misaligned",
            ));
        }
        let mut end = self.core_end;
        for h in self.headers.chunks_exact(12) {
            let p = u32_at(h, 4)? as usize;
            if p > start {
                end = end.min(p);
            }
        }
        d.data = self.data.get(start..end).ok_or("payload offset")?;
        let count = usize::try_from(u32_at(d.data, 0)?)
            .map_err(|_| Error::InvalidData("bone count overflow"))?;
        d.names = slice(
            d.data,
            4,
            count.checked_mul(2).ok_or("name table size overflow")?,
        )?;
        let matrix_start = 4usize
            .checked_add(d.names.len())
            .and_then(|n| n.checked_add(3))
            .map(|n| n & !3)
            .ok_or("matrix offset overflow")?;
        d.matrices = slice(
            d.data,
            matrix_start,
            count.checked_mul(48).ok_or("matrix table size overflow")?,
        )?;
        Ok(d)
    }
    pub fn find_deformer(self, id: u16) -> Result<Option<Deformer<'a>>> {
        for i in 0..self.deformer_count() {
            if self.deformer_id(i)? == id {
                return self.deformer(i).map(Some);
            }
        }
        Ok(None)
    }
    /// Query a path between **deformer table indices**, not skeleton IDs or
    /// connection indices. Yields the descendant first, excludes the ancestor.
    /// Equal indices return an empty path; `None` confirms no ancestor relation.
    /// Invalid indices and budget exhaustion are errors, never `None`.
    ///
    /// `max_steps` counts logical record reads: two initial header lookups,
    /// then two per visited connection, reserving its preflight and iteration
    /// reads before returning the iterator. No path allocation is performed.
    /// Bone lookup, matrix composition, race fallback and EPBD condition choice
    /// are separate operations, not implied by a path existing.
    pub fn ancestor_path(
        self,
        descendant: usize,
        ancestor: usize,
        max_steps: usize,
    ) -> Result<Option<AncestorPath<'a>>> {
        let n = self.deformer_count();
        if descendant >= n || ancestor >= n {
            return Err(Error::IndexOutOfBounds);
        }
        let mut budget = max_steps;
        spend(&mut budget, 2, "ancestry query")?;
        let start = usize::from(u16_at(record(self.headers, descendant, 12)?, 2)?);
        let stop = usize::from(u16_at(record(self.headers, ancestor, 12)?, 2)?);
        let mut cursor = Some(start);
        let mut length = 0;
        for _ in 0..=n {
            if cursor == Some(stop) {
                return Ok(Some(AncestorPath {
                    pbd: self,
                    cursor: Some(start),
                    remaining: length,
                }));
            }
            let Some(at) = cursor else {
                return Ok(None);
            };
            spend(&mut budget, 2, "ancestry query")?;
            cursor = self.connection(at)?.parent;
            length += 1;
        }
        Err(Error::InvalidData("ancestry parent cycle"))
    }
    pub fn connection(self, index: usize) -> Result<Connection> {
        let r = record(self.connections, index, 8)?;
        let link = |p| -> Result<Option<usize>> {
            let x = u16_at(r, p)? as i16;
            if x == -1 {
                return Ok(None);
            }
            if x < 0 || x as usize >= self.deformer_count() {
                return Err(Error::InvalidData("connection index out of range"));
            }
            Ok(Some(x as usize))
        };
        let deformer_index = link(6)?.ok_or("missing deformer index")?;
        Ok(Connection {
            parent: link(0)?,
            first_child: link(2)?,
            next_sibling: link(4)?,
            deformer_index,
        })
    }
    pub fn extension(self) -> Option<Extended<'a>> {
        self.extension
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Extended<'a> {
    table: flat::Table<'a>,
    entries: &'a [u8],
    version_tag: Option<u16>,
    unknown_root_scalar: Option<u16>,
}
impl<'a> Extended<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        if slice(data, 4, 4)? != b"EPBD" {
            return Err(Error::InvalidData("EPBD identifier"));
        }
        let table = flat::Table::new(data, flat::indirect(data, 0)?)?;
        let version_tag = table.optional_ushort(0)?;
        let unknown_root_scalar = table.optional_ushort(1)?;
        let entries = table.vector(2, 4)?;
        Ok(Self {
            table,
            entries,
            version_tag,
            unknown_root_scalar,
        })
    }
    /// Root field 0, as stored. The verified Win64 loader accepts omission or
    /// the value 1; this accessor does not synthesize that consumer default or
    /// reject other stored values as a structural parsing error.
    pub fn version_tag(self) -> Option<u16> {
        self.version_tag
    }
    /// Optional u16 root field 1; its semantic meaning is unconfirmed.
    pub fn unknown_root_scalar(self) -> Option<u16> {
        self.unknown_root_scalar
    }
    pub fn entry_count(self) -> usize {
        self.entries.len() / 4
    }
    pub fn raw_flatbuffer(self) -> &'a [u8] {
        self.table.data
    }
    /// Find a unique entry by its exact raw u32 child/parent race pair.
    ///
    /// This searches file metadata, not native routing: no low-u16 key coercion,
    /// selector choice, fallback or matrix application is performed. Duplicate
    /// pairs are a query ambiguity, not a structural parse prohibition. A match
    /// is returned only after scanning all entries to establish uniqueness.
    /// Each entry consumes one unit of `max_entries`; exhaustion is an error,
    /// not confirmed absence or an early successful match.
    pub fn find_entry(
        self,
        child_race: u32,
        parent_race: u32,
        max_entries: usize,
    ) -> Result<Option<ExtendedEntry<'a>>> {
        let mut remaining = max_entries;
        let mut found = None;
        for i in 0..self.entry_count() {
            spend(&mut remaining, 1, "EPBD entry query")?;
            let entry = self.entry(i)?;
            if (entry.child_race, entry.parent_race) == (child_race, parent_race) {
                if found.is_some() {
                    return Err(Error::InvalidData("ambiguous EPBD race pair"));
                }
                found = Some(entry);
            }
        }
        Ok(found)
    }
    pub fn entry(self, index: usize) -> Result<ExtendedEntry<'a>> {
        if index >= self.entry_count() {
            return Err(Error::IndexOutOfBounds);
        }
        let base = self.entries.as_ptr() as usize - self.table.data.as_ptr() as usize;
        let table = flat::Table::new(
            self.table.data,
            flat::indirect(self.table.data, base + index * 4)?,
        )?;
        Ok(ExtendedEntry {
            child_race: table.uint(0)?,
            parent_race: table.uint(1)?,
            conditions: table.vector(2, 4)?,
            pbd: table.vector(3, 1)?,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ExtendedEntry<'a> {
    pub child_race: u32,
    pub parent_race: u32,
    conditions: &'a [u8],
    pbd: &'a [u8],
}
impl<'a> ExtendedEntry<'a> {
    /// Raw u32 selector vector (historically named conditions). In the verified
    /// Win64 profile the low u16 of each value is an embedded-PBD skeleton ID,
    /// not a bitmask. Upper bits are preserved by this raw view.
    pub fn raw_conditions(self) -> &'a [u8] {
        self.conditions
    }
    pub fn conditions(self) -> impl ExactSizeIterator<Item = u32> + 'a {
        self.conditions
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }
    /// Embedded-PBD skeleton IDs used by the verified Win64 selector path.
    /// This exposes the low u16 of each raw selector; it does not choose an
    /// entry, apply native fallback or equate these IDs with outer race codes.
    pub fn local_skeleton_ids(self) -> impl ExactSizeIterator<Item = u16> + 'a {
        self.conditions().map(|value| value as u16)
    }
    pub fn pbd_bytes(self) -> &'a [u8] {
        self.pbd
    }
    /// The containing parse validated this embedded PBD under shared limits.
    pub fn pbd(self) -> Result<Pbd<'a>> {
        Pbd::skeleton(self.pbd)
    }
}

fn pack(bytes: &[u8]) -> Result<(usize, Option<Extended<'_>>)> {
    if bytes.len() < 24 || &bytes[bytes.len() - 24..bytes.len() - 20] != b"PACK" {
        return Ok((bytes.len(), None));
    }
    let footer = &bytes[bytes.len() - 24..];
    if u16_at(footer, 4)? != 1 || u16_at(footer, 6)? != 1 {
        return Err(Error::UnsupportedPack);
    }
    let wide = |b: &[u8], at| -> Result<u64> {
        let s = slice(b, at, 8)?;
        Ok(u64::from_le_bytes(
            s.try_into().map_err(|_| Error::InvalidData("u64"))?,
        ))
    };
    let total =
        usize::try_from(wide(footer, 16)?).map_err(|_| Error::InvalidData("PACK size overflow"))?;
    let start = bytes
        .len()
        .checked_sub(total)
        .ok_or("PACK size exceeds input")?;
    if total < 40 {
        return Err(Error::InvalidData("PACK size too small"));
    }
    let head = slice(bytes, start, 16)?;
    if &head[..4] != b"EPBD" || u16_at(head, 4)? != 1 || u16_at(head, 6)? != 0 {
        return Err(Error::UnsupportedPack);
    }
    let payload = total - 40;
    if wide(head, 8)? != payload as u64
        || wide(footer, 8)? as i64
            != -(i64::try_from(total - 24).map_err(|_| Error::InvalidData("PACK back offset"))?)
    {
        return Err(Error::InvalidData("PACK length/back offset mismatch"));
    }
    Ok((
        start,
        Some(Extended::parse(slice(bytes, start + 16, payload)?)?),
    ))
}
