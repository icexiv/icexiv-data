use super::{Avfx, Block, Curve, Error, Fields, Kind, Node, Result};

#[derive(Clone, Copy, Debug)]
pub struct AssetLimits {
    pub max_blocks: usize,
    pub max_items: usize,
    pub max_output_bytes: usize,
    pub max_work: usize,
}
impl Default for AssetLimits {
    fn default() -> Self {
        Self {
            max_blocks: 1_000_000,
            max_items: 1_000_000,
            max_output_bytes: 64 << 20,
            max_work: 16_000_000,
        }
    }
}
struct Budget {
    limits: AssetLimits,
    blocks: usize,
    items: usize,
    bytes: usize,
    work: usize,
}
impl Budget {
    fn new(l: AssetLimits) -> Self {
        Self {
            limits: l,
            blocks: l.max_blocks,
            items: l.max_items,
            bytes: l.max_output_bytes,
            work: l.max_work,
        }
    }
    fn work(&mut self, n: usize) -> Result<()> {
        self.work = self.work.checked_sub(n).ok_or(Error::BudgetExceeded)?;
        Ok(())
    }
    fn fields<'a>(&mut self, b: Block<'a>) -> Result<Fields<'a>> {
        let f = b.fields(self.blocks)?;
        self.blocks = self
            .blocks
            .checked_sub(f.len())
            .ok_or(Error::BudgetExceeded)?;
        self.work(f.len())?;
        Ok(f)
    }
    fn reserve<T>(&mut self, n: usize) -> Result<Vec<T>> {
        self.items = self.items.checked_sub(n).ok_or(Error::BudgetExceeded)?;
        let size = n
            .checked_mul(std::mem::size_of::<T>())
            .ok_or("asset allocation overflow")?;
        self.bytes = self.bytes.checked_sub(size).ok_or(Error::BudgetExceeded)?;
        let mut v = Vec::new();
        v.try_reserve_exact(n)
            .map_err(|_| Error::AllocationFailed)?;
        Ok(v)
    }
    fn push<T>(&mut self, v: &mut Vec<T>, x: T) -> Result<()> {
        self.items = self.items.checked_sub(1).ok_or(Error::BudgetExceeded)?;
        if v.len() == v.capacity() {
            let add = v.capacity().max(8);
            let size = add
                .checked_mul(std::mem::size_of::<T>())
                .ok_or("asset allocation overflow")?;
            self.bytes = self.bytes.checked_sub(size).ok_or(Error::BudgetExceeded)?;
            v.try_reserve_exact(add)
                .map_err(|_| Error::AllocationFailed)?;
        }
        v.push(x);
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataKind {
    EmitterCone,
    EmitterConeModel,
    EmitterSphere,
    EmitterCylinder,
    EmitterModel,
    ParticlePowder,
    ParticleWindmill,
    ParticleModel,
    ParticleDisc,
    ParticleLightModel,
    ParticleLine,
    ParticleLaser,
    ParticlePolyline,
    ParticleQuad,
    ParticleDecal,
    ParticleDecalRing,
    ParticleModelSkin,
    ParticleDissolve,
    EffectorPointLight,
    EffectorDirectionalLight,
    EffectorChromaticAberration,
    EffectorGaussianBlur,
    EffectorDirectionalBlur,
    EffectorCameraQuakeVariable,
    EffectorRadialBlur,
    EffectorCameraQuake,
    BinderPoint,
    BinderLinear,
    BinderSpline,
    BinderCamera,
    BinderLinearAdjust,
}
#[derive(Clone, Copy, Debug)]
pub enum ParameterValue<'a> {
    Integer(i32),
    Byte(u8),
    Real(f32),
    Bytes(&'a [u8]),
    Curve(Curve<'a>),
    Group(Fields<'a>),
    Unknown(Block<'a>),
}
#[derive(Clone, Copy, Debug)]
pub struct Parameter<'a> {
    pub block: Block<'a>,
    pub value: ParameterValue<'a>,
}
#[derive(Debug)]
pub enum DataView<'a> {
    Absent,
    Known {
        kind: DataKind,
        block: Block<'a>,
        parameters: Vec<Parameter<'a>>,
    },
    Unknown {
        node_kind: Kind,
        variant: Option<i32>,
        block: Block<'a>,
    },
}
enum Encoding {
    Int,
    Real,
    Bytes,
    Curve,
    Group,
    SmallInt,
    Unknown,
}
fn encoding(kind: DataKind, b: Block<'_>) -> Encoding {
    use DataKind::*;
    use Encoding::*;
    let is = |tags: &[&[u8]]| tags.iter().any(|t| b.is(t));
    match kind {
        EmitterSphere | EmitterCylinder | EmitterModel | EmitterConeModel => {
            if is(&[b"ROT", b"GeMT"])
                || (kind == EmitterModel && b.is(b"MdNo"))
                || (kind != EmitterModel && is(&[b"DivX", b"DivY"]))
            {
                Int
            } else if is(&[
                b"IjS", b"IjSR", b"AnX", b"AnY", b"AnZ", b"AnXR", b"AnYR", b"AnZR",
            ]) || (kind == EmitterSphere && b.is(b"Rads"))
                || (kind == EmitterCylinder && is(&[b"Len", b"Rad"]))
                || (kind == EmitterConeModel && is(&[b"Rad", b"RadR", b"IjA", b"IjAR"]))
            {
                Curve
            } else {
                Unknown
            }
        }
        EmitterCone => {
            if b.is(b"ROT") {
                Int
            } else if is(&[
                b"AnX", b"AnY", b"AnZ", b"AnXR", b"AnYR", b"AnZR", b"InS", b"InSR", b"OuS",
                b"OuSR", b"IjS", b"IjSR", b"IjA", b"IjAR",
            ]) {
                Curve
            } else {
                Unknown
            }
        }
        ParticleModelSkin => {
            if is(&[b"FrsT", b"AuTT", b"bCM"]) {
                Int
            } else if is(&[b"FrC", b"FrCR", b"SEM", b"SEMR", b"EEM", b"EEMR"]) {
                Curve
            } else if is(&[b"FrRt", b"ColB", b"ColE", b"UVPD"]) {
                Group
            } else {
                Unknown
            }
        }
        ParticleLine => {
            if b.is(b"LnCT") {
                Int
            } else if is(&[b"Len", b"LenR"]) {
                Curve
            } else if is(&[b"ColB", b"ColE"]) {
                Group
            } else {
                Unknown
            }
        }
        ParticleLaser => {
            if is(&[b"Len", b"LenR", b"Wdt", b"WdtR"]) {
                Curve
            } else {
                Unknown
            }
        }
        ParticleQuad => {
            if b.is(b"bMP") {
                SmallInt
            } else if b.is(b"SS") {
                Int
            } else {
                Unknown
            }
        }
        ParticleDecal | ParticleDecalRing => {
            if b.is(b"DDTT") {
                Int
            } else if b.is(b"SS") || (kind == ParticleDecalRing && b.is(b"RF")) {
                Real
            } else if kind == ParticleDecalRing && is(&[b"WID", b"WIDR"]) {
                Curve
            } else {
                Unknown
            }
        }
        ParticleDissolve => {
            if is(&[b"bRev", b"BST", b"NPT", b"DTT"]) {
                Int
            } else if is(&[b"EroR", b"EdW", b"EdCW", b"ECMP", b"Int"]) {
                Curve
            } else if b.is(b"EdC") {
                Group
            } else {
                Unknown
            }
        }
        ParticlePolyline => {
            if is(&[
                b"LnCT", b"NBBA", b"BWpT", b"PnC", b"PnCC", b"PnED", b"bEdg", b"bNtB", b"BdWp",
                b"bCtg", b"bCtr", b"TagN", b"bSpl", b"bLcl",
            ]) {
                Int
            } else if is(&[
                b"CF", b"CFR", b"Wd", b"WdR", b"WdB", b"WdBR", b"WdC", b"WdCR", b"WdE", b"WdER",
                b"Len", b"LenR", b"Sft", b"SftR", b"PnDs",
            ]) {
                Curve
            } else if is(&[b"ColB", b"ColC", b"ColE"]) {
                Group
            } else {
                Unknown
            }
        }
        EffectorDirectionalLight => {
            if is(&[b"Pow", b"PowR"]) {
                Curve
            } else if is(&[b"Amb", b"Col", b"Rot"]) {
                Group
            } else {
                Unknown
            }
        }
        EffectorCameraQuake | EffectorCameraQuakeVariable => {
            if is(&[b"Att", b"AttR", b"RdO", b"RdOR", b"RdI", b"RdIR"]) {
                Curve
            } else if is(&[b"Rot", b"Pos"]) {
                Group
            } else {
                Unknown
            }
        }
        EffectorChromaticAberration
        | EffectorGaussianBlur
        | EffectorDirectionalBlur
        | EffectorRadialBlur => {
            if b.is(b"FaBP") || (kind == EffectorDirectionalBlur && b.is(b"bOS")) {
                Int
            } else if is(&[b"FSDc", b"FEDc"]) {
                Real
            } else if (kind == EffectorDirectionalBlur && is(&[b"Len", b"Str", b"AStr", b"Ang"]))
                || (kind != EffectorDirectionalBlur && is(&[b"Str", b"Gra", b"IRad", b"ORad"]))
                || (kind == EffectorChromaticAberration && is(&[b"CSR", b"CSS", b"CGE"]))
                || (matches!(kind, EffectorGaussianBlur | EffectorRadialBlur) && b.is(b"Len"))
            {
                Curve
            } else {
                Unknown
            }
        }
        BinderLinear | BinderSpline => {
            if is(&[b"COF", b"COFR"]) {
                Curve
            } else {
                Unknown
            }
        }
        BinderLinearAdjust => {
            if b.is(b"COF") {
                Curve
            } else {
                Unknown
            }
        }
        BinderCamera => {
            if is(&[b"Dst", b"DstR"]) {
                Curve
            } else {
                Unknown
            }
        }
        ParticlePowder => {
            if is(&[b"bMV", b"bLoc", b"bLgt"]) {
                SmallInt
            } else if b.is(b"LgtT") {
                Int
            } else if b.is(b"CnOf") {
                Real
            } else {
                Unknown
            }
        }
        ParticleWindmill => {
            if b.is(b"WUvT") {
                Int
            } else {
                Unknown
            }
        }
        ParticleModel => {
            if is(&[
                b"MNRv", b"MNRt", b"MNRi", b"FrsT", b"DLT", b"PLT", b"bLgt", b"bShp",
            ]) {
                Int
            } else if b.is(b"MdNo") {
                Bytes
            } else if is(&[b"NoAn", b"Moph", b"FrC", b"FrCR"]) {
                Curve
            } else if is(&[b"FrRt", b"ColB", b"ColE"]) {
                Group
            } else {
                Unknown
            }
        }
        ParticleLightModel => {
            if b.is(b"MNO") {
                SmallInt
            } else {
                Unknown
            }
        }
        ParticleDisc => {
            if is(&[b"PrtC", b"PCnU", b"PCnV", b"SS"]) {
                Int
            } else if b.is(b"PIFU") {
                Real
            } else if is(&[b"CEI", b"CEO"]) {
                Group
            } else if is(&[
                b"Ang", b"AngR", b"HBI", b"HBIR", b"HEI", b"HEIR", b"HBO", b"HBOR", b"HEO",
                b"HEOR", b"WB", b"WBR", b"WE", b"WER", b"RB", b"RBR", b"RE", b"RER",
            ]) {
                Curve
            } else {
                Unknown
            }
        }
        EffectorPointLight => {
            if is(&[b"Attn", b"bSdw", b"bChS", b"bMpS", b"bMvS"]) {
                Int
            } else if is(&[b"SCDN", b"SCDF"]) {
                Real
            } else if b.is(b"DstS") {
                Curve
            } else if is(&[b"Col", b"Rot", b"Pos"]) {
                Group
            } else {
                Unknown
            }
        }
        BinderPoint => {
            if is(&[b"SpS", b"SpSR"]) {
                Curve
            } else {
                Unknown
            }
        }
    }
}
fn kind(n: Node<'_>, variant: Option<i32>) -> Option<DataKind> {
    use DataKind::*;
    Some(match (n.kind, variant?) {
        (Kind::Emitter, 1) => EmitterCone,
        (Kind::Emitter, 2) => EmitterConeModel,
        (Kind::Emitter, 3) => EmitterSphere,
        (Kind::Emitter, 4) => EmitterCylinder,
        (Kind::Emitter, 5) => EmitterModel,
        (Kind::Particle, 1) => ParticlePowder,
        (Kind::Particle, 2) => ParticleWindmill,
        (Kind::Particle, 3) => ParticleLine,
        (Kind::Particle, 4) => ParticleLaser,
        (Kind::Particle, 5) => ParticleModel,
        (Kind::Particle, 6) => ParticlePolyline,
        (Kind::Particle, 8) => ParticleQuad,
        (Kind::Particle, 10) => ParticleDecal,
        (Kind::Particle, 11) => ParticleDecalRing,
        (Kind::Particle, 12) => ParticleDisc,
        (Kind::Particle, 13) => ParticleLightModel,
        (Kind::Particle, 14) => ParticleModelSkin,
        (Kind::Particle, 15) => ParticleDissolve,
        (Kind::Effector, 0) => EffectorPointLight,
        (Kind::Effector, 1) => EffectorDirectionalLight,
        (Kind::Effector, 3) => EffectorChromaticAberration,
        (Kind::Effector, 4) => EffectorGaussianBlur,
        (Kind::Effector, 5) => EffectorDirectionalBlur,
        (Kind::Effector, 6) => EffectorCameraQuakeVariable,
        (Kind::Effector, 7) => EffectorRadialBlur,
        (Kind::Effector, 9) => EffectorCameraQuake,
        (Kind::Binder, 0) => BinderPoint,
        (Kind::Binder, 1) => BinderLinear,
        (Kind::Binder, 2) => BinderSpline,
        (Kind::Binder, 3) => BinderCamera,
        (Kind::Binder, 4) => BinderLinearAdjust,
        _ => return None,
    })
}
fn data<'a>(n: Node<'a>, budget: &mut Budget) -> Result<DataView<'a>> {
    budget.work(n.fields.len().checked_mul(2).ok_or("asset work overflow")?)?;
    let variant = if n.kind == Kind::Binder {
        n.fields.integer(b"BnVr")?
    } else {
        n.variant()?
    };
    let Some(block) = n.fields.one(b"Data")? else {
        return Ok(DataView::Absent);
    };
    let Some(kind) = kind(n, variant) else {
        return Ok(DataView::Unknown {
            node_kind: n.kind,
            variant,
            block,
        });
    };
    let fields = budget.fields(block)?;
    let mut parameters = budget.reserve(fields.len())?;
    for b in fields.iter() {
        let value = match encoding(kind, b) {
            Encoding::Int => ParameterValue::Integer(b.i32()?),
            Encoding::Real => ParameterValue::Real(b.f32()?),
            Encoding::Bytes => ParameterValue::Bytes(b.payload),
            Encoding::SmallInt => {
                if b.payload.len() == 1 {
                    ParameterValue::Byte(b.byte()?)
                } else {
                    ParameterValue::Integer(b.i32()?)
                }
            }
            Encoding::Curve => {
                let f = budget.fields(b)?;
                budget.work(f.len())?;
                let c = Curve::parse(b, budget.limits.max_blocks)?;
                budget.work(c.keys().len())?;
                ParameterValue::Curve(c)
            }
            Encoding::Group => ParameterValue::Group(budget.fields(b)?),
            Encoding::Unknown => ParameterValue::Unknown(b),
        };
        parameters.push(Parameter { block: b, value });
    }
    Ok(DataView::Known {
        kind,
        block,
        parameters,
    })
}
#[derive(Clone, Copy, Debug)]
pub struct EmitterItem<'a> {
    pub target_kind: Kind,
    pub fields: Fields<'a>,
    pub enabled: Option<i32>,
    pub target_index: Option<i32>,
    pub create_time: Option<i32>,
    pub create_count: Option<i32>,
    pub create_probability: Option<i32>,
}
fn emitter_items<'a>(node: Node<'a>, b: &mut Budget) -> Result<Vec<EmitterItem<'a>>> {
    if node.kind != Kind::Emitter {
        return Err("not an emitter".into());
    }
    b.work(
        node.fields
            .len()
            .checked_mul(3)
            .ok_or("emitter work overflow")?,
    )?;
    let emit = usize::try_from(
        node.fields
            .integer(b"EmCn")?
            .ok_or("missing emitter item count")?,
    )
    .map_err(|_| "negative emitter count")?;
    let particle = usize::try_from(
        node.fields
            .integer(b"PrCn")?
            .ok_or("missing particle item count")?,
    )
    .map_err(|_| "negative particle count")?;
    let total = emit
        .checked_add(particle)
        .ok_or("emitter item count overflow")?;
    let mut seen_emit = 0usize;
    let mut seen_particle = 0usize;
    for field in node.fields.iter() {
        if field.is(b"ItEm") {
            seen_emit += 1;
        } else if field.is(b"ItPr") {
            seen_particle += 1;
        }
    }
    if seen_emit != emit || seen_particle != particle {
        return Err("emitter item block counts".into());
    }
    let mut output = b.reserve(total)?;
    for block in node.fields.iter() {
        let target_kind = if block.is(b"ItEm") {
            Kind::Emitter
        } else if block.is(b"ItPr") {
            Kind::Particle
        } else {
            continue;
        };
        let fs = b.fields(block)?;
        let mut start = None;
        let mut count = 0;
        // Each outer block appends its own record. The repeated prefix can
        // contain either category, depending on actual wire order.
        for field in fs.iter() {
            if field.is(b"bEnb") {
                start = Some(field.offset);
                count = 0;
            } else if start.is_none() {
                return Err("emitter field before bEnb".into());
            }
            count += 1;
        }
        let start = start.ok_or("empty emitter item")?;
        let end = fs
            .origin
            .checked_add(fs.bytes.len())
            .ok_or("item end overflow")?;
        let relative = start.checked_sub(fs.origin).ok_or("item origin")?;
        let len = end.checked_sub(start).ok_or("item span")?;
        let fields = Fields {
            bytes: fs
                .bytes
                .get(relative..relative.checked_add(len).ok_or("item span overflow")?)
                .ok_or("item range")?,
            origin: start,
            count,
        };
        if output.len() >= total {
            return Err("too many emitter items".into());
        }
        b.work(count.checked_mul(5).ok_or("item work overflow")?)?;
        output.push(EmitterItem {
            target_kind,
            fields,
            enabled: fields.integer(b"bEnb")?,
            target_index: fields.integer(b"TgtB")?,
            create_time: fields.integer(b"CrTm")?,
            create_count: fields.integer(b"CrCn")?,
            create_probability: fields.integer(b"CrPr")?,
        });
    }
    if output.len() != total {
        return Err("emitter item count mismatch".into());
    }
    Ok(output)
}
#[derive(Clone, Copy, Debug)]
pub struct TextureSlot<'a> {
    pub block: Block<'a>,
    pub fields: Fields<'a>,
    pub enabled: Option<i32>,
    pub texture_index: Option<i32>,
    pub uv_set: Option<i32>,
    pub mask_indices: Option<&'a [u8]>,
    pub texture_curve: Option<Curve<'a>>,
}
fn texture_slots<'a>(n: Node<'a>, b: &mut Budget) -> Result<Vec<TextureSlot<'a>>> {
    if n.kind != Kind::Particle {
        return Err("not a particle".into());
    }
    b.work(n.fields.len())?;
    let blocks = n
        .fields
        .iter()
        .filter(|v| {
            [
                b"TC1" as &[u8],
                b"TC2",
                b"TC3",
                b"TC4",
                b"TN",
                b"TR",
                b"TD",
                b"TP",
            ]
            .iter()
            .any(|tag| v.is(tag))
        })
        .count();
    let mut output = b.reserve(blocks)?;
    for block in n.fields.iter() {
        if ![
            b"TC1" as &[u8],
            b"TC2",
            b"TC3",
            b"TC4",
            b"TN",
            b"TR",
            b"TD",
            b"TP",
        ]
        .iter()
        .any(|tag| block.is(tag))
        {
            continue;
        }
        let f = b.fields(block)?;
        b.work(f.len().checked_mul(5).ok_or("texture work overflow")?)?;
        let curve = if let Some(curve) = f.one(b"TxN")? {
            let nested = b.fields(curve)?;
            b.work(nested.len())?;
            Some(Curve::parse(curve, b.limits.max_blocks)?)
        } else {
            None
        };
        output.push(TextureSlot {
            block,
            fields: f,
            enabled: f.integer(b"bEna")?,
            texture_index: f.integer(b"TxNo")?,
            uv_set: f.integer(b"UvSN")?,
            mask_indices: f.one(b"TLst")?.map(|v| v.payload),
            texture_curve: curve,
        });
    }
    Ok(output)
}
impl<'a> Node<'a> {
    pub fn data_view(self, limits: AssetLimits) -> Result<DataView<'a>> {
        data(self, &mut Budget::new(limits))
    }
    /// One appended record per ItEm/ItPr block, in wire order. Its outer tag
    /// determines the target index space, including particle-first interleaving.
    pub fn emitter_items(self, limits: AssetLimits) -> Result<Vec<EmitterItem<'a>>> {
        emitter_items(self, &mut Budget::new(limits))
    }
    pub fn texture_slots(self, limits: AssetLimits) -> Result<Vec<TextureSlot<'a>>> {
        texture_slots(self, &mut Budget::new(limits))
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceKind {
    Timeline,
    Emitter,
    Particle,
    Effector,
    Binder,
    Model,
    Texture,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceStatus {
    InRange,
    Negative,
    OutOfRange,
}
#[derive(Clone, Copy, Debug)]
pub struct Reference {
    pub owner_offset: usize,
    pub field_offset: usize,
    pub element: usize,
    pub kind: ReferenceKind,
    pub raw_index: i32,
    pub status: ReferenceStatus,
}
#[derive(Debug)]
pub struct AssetReport {
    pub references: Vec<Reference>,
    pub animated_texture_curves: usize,
    pub unknown_data_nodes: usize,
}
impl Avfx<'_> {
    /// Range observations, not execution validity. Negative values remain
    /// unresolved (not confirmed absence); even disabled texture slots retain
    /// their raw references. Animated selection is counted, never collapsed.
    pub fn asset_report(&self, limits: AssetLimits) -> Result<AssetReport> {
        let mut b = Budget::new(limits);
        let mut report = AssetReport {
            references: Vec::new(),
            animated_texture_curves: 0,
            unknown_data_nodes: 0,
        };
        let counts = [
            Kind::Timeline,
            Kind::Emitter,
            Kind::Particle,
            Kind::Effector,
            Kind::Binder,
        ]
        .map(|kind| self.nodes_of(kind).count());
        b.work(
            self.nodes
                .len()
                .checked_mul(5)
                .ok_or("node count work overflow")?,
        )?;
        let target_count = |kind| match kind {
            ReferenceKind::Timeline => counts[0],
            ReferenceKind::Emitter => counts[1],
            ReferenceKind::Particle => counts[2],
            ReferenceKind::Effector => counts[3],
            ReferenceKind::Binder => counts[4],
            ReferenceKind::Model => self.models.len(),
            ReferenceKind::Texture => self.textures.len(),
        };
        let add = |report: &mut AssetReport,
                   b: &mut Budget,
                   owner: usize,
                   field: usize,
                   element: usize,
                   kind: ReferenceKind,
                   index: i32|
         -> Result<()> {
            b.work(1)?;
            let status = if index < 0 {
                ReferenceStatus::Negative
            } else if usize::try_from(index).is_ok_and(|v| v < target_count(kind)) {
                ReferenceStatus::InRange
            } else {
                ReferenceStatus::OutOfRange
            };
            b.push(
                &mut report.references,
                Reference {
                    owner_offset: owner,
                    field_offset: field,
                    element,
                    kind,
                    raw_index: index,
                    status,
                },
            )
        };
        for n in &self.nodes {
            b.work(n.fields.len())?;
            match n.kind {
                Kind::Scheduler | Kind::Timeline => {
                    for tag in [b"Item" as &[u8], b"Trgr"] {
                        let mut last = None;
                        for f in n.fields.iter() {
                            if f.is(tag) {
                                last = Some(f);
                            }
                        }
                        if let Some(last) = last {
                            for f in b.fields(last)?.iter() {
                                let target = if f.is(b"TlNo") {
                                    Some(ReferenceKind::Timeline)
                                } else if f.is(b"EmNo") {
                                    Some(ReferenceKind::Emitter)
                                } else if f.is(b"EfNo") {
                                    Some(ReferenceKind::Effector)
                                } else if f.is(b"BdNo") {
                                    Some(ReferenceKind::Binder)
                                } else {
                                    None
                                };
                                if let Some(kind) = target {
                                    add(
                                        &mut report,
                                        &mut b,
                                        n.block.offset,
                                        f.offset,
                                        0,
                                        kind,
                                        f.i32()?,
                                    )?;
                                }
                            }
                        }
                    }
                }
                Kind::Emitter => {
                    if let Some(f) = n.fields.one(b"EfNo")? {
                        add(
                            &mut report,
                            &mut b,
                            n.block.offset,
                            f.offset,
                            0,
                            ReferenceKind::Effector,
                            f.i32()?,
                        )?;
                    }
                    for item in emitter_items(*n, &mut b)? {
                        if let Some(f) = item.fields.one(b"TgtB")? {
                            add(
                                &mut report,
                                &mut b,
                                n.block.offset,
                                f.offset,
                                0,
                                if item.target_kind == Kind::Emitter {
                                    ReferenceKind::Emitter
                                } else {
                                    ReferenceKind::Particle
                                },
                                f.i32()?,
                            )?;
                        }
                    }
                }
                Kind::Particle => {
                    for slot in texture_slots(*n, &mut b)? {
                        report.animated_texture_curves += usize::from(slot.texture_curve.is_some());
                        if let Some(f) = slot.fields.one(b"TxNo")? {
                            add(
                                &mut report,
                                &mut b,
                                n.block.offset,
                                f.offset,
                                0,
                                ReferenceKind::Texture,
                                f.i32()?,
                            )?;
                        }
                        if let Some(f) = slot.fields.one(b"TLst")? {
                            for (i, v) in f.payload.iter().enumerate() {
                                add(
                                    &mut report,
                                    &mut b,
                                    n.block.offset,
                                    f.offset,
                                    i,
                                    ReferenceKind::Texture,
                                    i32::from(*v),
                                )?;
                            }
                        }
                    }
                }
                _ => (),
            }
            match data(*n, &mut b)? {
                DataView::Known {
                    kind, parameters, ..
                } => {
                    for p in parameters {
                        let target = match kind {
                            DataKind::EmitterModel if p.block.is(b"MdNo") => {
                                Some(ReferenceKind::Model)
                            }
                            DataKind::ParticleModel if p.block.is(b"MdNo") => {
                                Some(ReferenceKind::Model)
                            }
                            DataKind::ParticleLightModel if p.block.is(b"MNO") => {
                                Some(ReferenceKind::Model)
                            }
                            _ => None,
                        };
                        if let Some(target) = target {
                            match p.value {
                                ParameterValue::Integer(v) => add(
                                    &mut report,
                                    &mut b,
                                    n.block.offset,
                                    p.block.offset,
                                    0,
                                    target,
                                    v,
                                )?,
                                ParameterValue::Byte(v) => add(
                                    &mut report,
                                    &mut b,
                                    n.block.offset,
                                    p.block.offset,
                                    0,
                                    target,
                                    i32::from(v),
                                )?,
                                ParameterValue::Bytes(v) => {
                                    for (i, v) in v.iter().enumerate() {
                                        add(
                                            &mut report,
                                            &mut b,
                                            n.block.offset,
                                            p.block.offset,
                                            i,
                                            target,
                                            i32::from(*v),
                                        )?;
                                    }
                                }
                                _ => return Err("model reference encoding".into()),
                            }
                        }
                    }
                }
                DataView::Unknown { .. } => report.unknown_data_nodes += 1,
                DataView::Absent => (),
            }
        }
        Ok(report)
    }
}
