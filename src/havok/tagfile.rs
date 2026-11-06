use super::{Error, Result, reserve, spend};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_allocation_bytes: usize,
    pub max_classes: usize,
    pub max_members: usize,
    pub max_objects: usize,
    pub max_strings: usize,
    pub max_string_bytes: usize,
    pub max_values: usize,
    pub max_array_length: usize,
    /// Values above 128 are rejected to bound recursive parsing/drop depth.
    pub max_depth: usize,
    pub max_validation_steps: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_allocation_bytes: 128 * 1024 * 1024,
            max_classes: 4096,
            max_members: 65536,
            max_objects: 65536,
            max_strings: 100000,
            max_string_bytes: 1024 * 1024,
            max_values: 2_000_000,
            max_array_length: 1_000_000,
            max_depth: 64,
            max_validation_steps: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Member<'a> {
    pub name: &'a [u8],
    pub raw_type: u32,
    pub tuple_size: usize,
    pub class_name: Option<&'a [u8]>,
}
#[derive(Debug)]
pub struct Class<'a> {
    pub name: &'a [u8],
    pub version: i64,
    pub parent: Option<usize>,
    pub members: Vec<Member<'a>>,
}
#[derive(Clone, Copy, Debug)]
pub struct FloatData<'a> {
    bytes: &'a [u8],
}
impl<'a> FloatData<'a> {
    pub fn raw(self) -> &'a [u8] {
        self.bytes
    }
    pub fn len(self) -> usize {
        self.bytes.len() / 4
    }
    pub fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }
    pub fn get(self, index: usize) -> Option<f32> {
        let p = index.checked_mul(4)?;
        let b = self.bytes.get(p..p.checked_add(4)?)?;
        Some(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn values(self) -> impl ExactSizeIterator<Item = f32> + 'a {
        self.bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}
#[derive(Debug)]
pub enum Value<'a> {
    Void,
    Byte(u8),
    Integer(i64),
    Real(f32),
    String(Option<&'a [u8]>),
    Reference(usize),
    Vector(FloatData<'a>),
    Bytes(&'a [u8]),
    Integers {
        width: u8,
        values: Vec<i64>,
    },
    Reals(FloatData<'a>),
    Vectors {
        components: usize,
        data: FloatData<'a>,
    },
    Struct(Object<'a>),
    Array(Vec<Value<'a>>),
}
#[derive(Debug)]
pub struct Field<'a> {
    pub name: &'a [u8],
    pub value: Value<'a>,
}
#[derive(Debug)]
pub struct Object<'a> {
    pub class_index: usize,
    pub fields: Vec<Field<'a>>,
}
impl<'a> Object<'a> {
    pub fn field(&self, name: &[u8]) -> Option<&Value<'a>> {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| &f.value)
    }
}
#[derive(Debug)]
pub enum Root<'a> {
    Inline(Object<'a>),
    Reference(usize),
    Null,
}
/// Class and object IDs retain their one-based file representation; ID zero is
/// null. References are checked after the whole stream, including forward refs.
#[derive(Debug)]
pub struct Tagfile<'a> {
    pub classes: Vec<Class<'a>>,
    pub objects: Vec<Object<'a>>,
    pub roots: Vec<Root<'a>>,
}
impl<'a> Tagfile<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::parse_with_limits(bytes, Limits::default())
    }
    pub fn parse_with_limits(bytes: &'a [u8], limits: Limits) -> Result<Self> {
        let (graph, consumed) = Self::parse_prefix_with_limits(bytes, limits)?;
        if consumed != bytes.len() {
            return Err(Error::InvalidData("trailing tagfile bytes"));
        }
        Ok(graph)
    }
    /// Container readers validate any bytes after the stream end themselves.
    pub(crate) fn parse_prefix_with_limits(
        bytes: &'a [u8],
        limits: Limits,
    ) -> Result<(Self, usize)> {
        if limits.max_depth > 128 {
            return Err(Error::BudgetExceeded("safe nesting ceiling"));
        }
        if bytes.len() > limits.max_input_bytes {
            return Err(Error::BudgetExceeded("input"));
        }
        let mut p = Parser {
            data: bytes,
            pos: 0,
            strings: Vec::new(),
            graph: Self {
                classes: Vec::new(),
                objects: Vec::new(),
                roots: Vec::new(),
            },
            limits,
            allocation: limits.max_allocation_bytes,
            values: limits.max_values,
            members: limits.max_members,
            steps: limits.max_validation_steps,
        };
        if p.take(8)? != [0x1e, 0x0d, 0xb0, 0xca, 0xce, 0xfa, 0x11, 0xd0] {
            return Err(Error::InvalidData("tagfile magic"));
        }
        if p.integer()? != 1 {
            return Err(Error::InvalidData("file info tag"));
        }
        let version = p.integer()?;
        if version != 3 {
            return Err(Error::UnsupportedVersion(version));
        }
        p.push_string(Some(&[]))?;
        p.push_string(None)?;
        loop {
            spend(&mut p.steps, 1, "tags")?;
            match p.integer()? {
                2 => p.class()?,
                tag @ (3 | 4) => {
                    if p.graph.objects.len() >= limits.max_objects {
                        return Err(Error::BudgetExceeded("objects"));
                    }
                    let class = p.count()?;
                    let mut values = p.structs(class, 1, 0, false)?;
                    let Value::Struct(obj) = values.pop().ok_or("missing object")? else {
                        return Err(Error::InvalidData("object value"));
                    };
                    let root = if tag == 4 {
                        grow(&mut p.graph.objects, &mut p.allocation)?;
                        p.graph.objects.push(obj);
                        Root::Reference(p.graph.objects.len())
                    } else {
                        Root::Inline(obj)
                    };
                    grow(&mut p.graph.roots, &mut p.allocation)?;
                    p.graph.roots.push(root);
                }
                5 => {
                    let index = p.count()?;
                    grow(&mut p.graph.roots, &mut p.allocation)?;
                    p.graph.roots.push(Root::Reference(index));
                }
                6 => {
                    grow(&mut p.graph.roots, &mut p.allocation)?;
                    p.graph.roots.push(Root::Null);
                }
                7 => break,
                _ => return Err(Error::InvalidData("unknown stream tag")),
            }
        }
        for i in 0..p.graph.classes.len() {
            for j in 0..p.graph.classes[i].members.len() {
                if let Some(name) = p.graph.classes[i].members[j].class_name {
                    p.class_id(name)?;
                }
            }
        }
        for root in &p.graph.roots {
            match root {
                Root::Reference(id) => {
                    p.graph.object(*id)?;
                }
                Root::Inline(o) => validate_object(o, &p.graph, &mut p.steps, 0, limits.max_depth)?,
                Root::Null => (),
            }
        }
        for obj in &p.graph.objects {
            validate_object(obj, &p.graph, &mut p.steps, 0, limits.max_depth)?;
        }
        Ok((p.graph, p.pos))
    }
    pub fn class(&self, id: usize) -> Result<&Class<'a>> {
        id.checked_sub(1)
            .and_then(|i| self.classes.get(i))
            .ok_or(Error::InvalidData("class reference"))
    }
    pub fn object(&self, id: usize) -> Result<Option<&Object<'a>>> {
        if id == 0 {
            Ok(None)
        } else {
            self.objects
                .get(id - 1)
                .map(Some)
                .ok_or(Error::InvalidData("object reference"))
        }
    }
}

fn grow<T>(v: &mut Vec<T>, allocation: &mut usize) -> Result<()> {
    if v.len() == v.capacity() {
        let extra = v.capacity().max(4);
        spend(
            allocation,
            extra
                .checked_mul(std::mem::size_of::<T>())
                .ok_or("allocation overflow")?,
            "allocation",
        )?;
        v.try_reserve_exact(extra)
            .map_err(|_| Error::AllocationFailed)?;
    }
    Ok(())
}
fn same_name(a: &[u8], b: &[u8], steps: &mut usize) -> Result<bool> {
    let cost = if a.len() == b.len() {
        a.len().checked_add(1).ok_or("name comparison overflow")?
    } else {
        1
    };
    spend(steps, cost, "name comparison")?;
    Ok(a == b)
}
struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    strings: Vec<Option<&'a [u8]>>,
    graph: Tagfile<'a>,
    limits: Limits,
    allocation: usize,
    values: usize,
    members: usize,
    steps: usize,
}
impl<'a> Parser<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("span overflow")?;
        let b = self.data.get(self.pos..end).ok_or("truncated tagfile")?;
        self.pos = end;
        Ok(b)
    }
    fn integer(&mut self) -> Result<i64> {
        let mut byte = self.take(1)?[0];
        let negative = byte & 1 != 0;
        let mut value = u64::from((byte >> 1) & 63);
        let mut shift = 6;
        while byte & 128 != 0 {
            if shift >= 64 {
                return Err(Error::InvalidData("packed integer too long"));
            }
            byte = self.take(1)?[0];
            let bits = u64::from(byte & 127);
            if bits > u64::MAX >> shift {
                return Err(Error::InvalidData("packed integer overflow"));
            }
            value |= bits << shift;
            shift += 7;
        }
        if negative && value == 1u64 << 63 {
            return Ok(i64::MIN);
        }
        let value =
            i64::try_from(value).map_err(|_| Error::InvalidData("packed integer magnitude"))?;
        Ok(if negative { -value } else { value })
    }
    fn count(&mut self) -> Result<usize> {
        usize::try_from(self.integer()?)
            .map_err(|_| Error::InvalidData("negative or overflowing count"))
    }
    fn push_string(&mut self, s: Option<&'a [u8]>) -> Result<()> {
        if self.strings.len() >= self.limits.max_strings {
            return Err(Error::BudgetExceeded("strings"));
        }
        grow(&mut self.strings, &mut self.allocation)?;
        self.strings.push(s);
        Ok(())
    }
    fn string(&mut self) -> Result<Option<&'a [u8]>> {
        let n = self.integer()?;
        if n <= 0 {
            return self
                .strings
                .get(
                    usize::try_from(n.checked_neg().ok_or("string index overflow")?)
                        .map_err(|_| Error::InvalidData("string index"))?,
                )
                .copied()
                .ok_or(Error::InvalidData("string reference"));
        }
        let n = usize::try_from(n).map_err(|_| Error::InvalidData("string size"))?;
        if n > self.limits.max_string_bytes {
            return Err(Error::BudgetExceeded("string length"));
        }
        let s = self.take(n)?;
        self.push_string(Some(s))?;
        Ok(Some(s))
    }
    fn class(&mut self) -> Result<()> {
        if self.graph.classes.len() >= self.limits.max_classes {
            return Err(Error::BudgetExceeded("classes"));
        }
        let name = self.string()?.ok_or("null class name")?;
        for class in &self.graph.classes {
            if same_name(class.name, name, &mut self.steps)? {
                return Err(Error::InvalidData("duplicate class name"));
            }
        }
        let version = self.integer()?;
        let parent = self.count()?;
        if parent > self.graph.classes.len() {
            return Err(Error::InvalidData("forward parent class"));
        }
        let n = self.count()?;
        spend(&mut self.members, n, "members")?;
        let mut members = reserve(n, &mut self.allocation)?;
        for _ in 0..n {
            let name = self.string()?.ok_or("null member name")?;
            let typ = self.count()?;
            if typ > 0x3f || typ & 15 > 10 {
                return Err(Error::UnsupportedType(typ as u32));
            }
            let tuple_size = if typ & 32 != 0 { self.count()? } else { 0 };
            if tuple_size > self.limits.max_array_length {
                return Err(Error::BudgetExceeded("tuple length"));
            }
            let class_name = if matches!(typ & 15, 8 | 9) {
                Some(self.string()?.ok_or("null member class")?)
            } else {
                None
            };
            members.push(Member {
                name,
                raw_type: typ as u32,
                tuple_size,
                class_name,
            });
        }
        grow(&mut self.graph.classes, &mut self.allocation)?;
        self.graph.classes.push(Class {
            name,
            version,
            parent: if parent == 0 { None } else { Some(parent) },
            members,
        });
        Ok(())
    }
    fn class_id(&mut self, name: &[u8]) -> Result<usize> {
        for (index, class) in self.graph.classes.iter().enumerate() {
            if same_name(class.name, name, &mut self.steps)? {
                return Ok(index + 1);
            }
        }
        Err(Error::InvalidData("unresolved class name"))
    }
    fn flat_members(&mut self, class: usize) -> Result<Vec<Member<'a>>> {
        let mut chain = reserve(
            self.limits.max_depth.min(self.graph.classes.len()),
            &mut self.allocation,
        )?;
        let mut at = Some(class);
        let mut total = 0usize;
        while let Some(id) = at {
            if chain.len() >= self.limits.max_depth {
                return Err(Error::BudgetExceeded("class ancestry"));
            }
            let c = self.graph.class(id)?;
            total = total
                .checked_add(c.members.len())
                .ok_or("member count overflow")?;
            at = c.parent;
            chain.push(id);
        }
        spend(&mut self.steps, total, "inherited members")?;
        let mut out = reserve(total, &mut self.allocation)?;
        for &id in chain.iter().rev() {
            out.extend_from_slice(&self.graph.class(id)?.members);
        }
        Ok(out)
    }
    fn depth(&self, depth: usize) -> Result<()> {
        if depth >= self.limits.max_depth {
            Err(Error::BudgetExceeded("value nesting"))
        } else {
            Ok(())
        }
    }
    fn value(&mut self, m: Member<'a>, depth: usize) -> Result<Value<'a>> {
        self.depth(depth)?;
        spend(&mut self.values, 1, "values")?;
        if m.raw_type & 16 != 0 {
            let n = self.count()?;
            return self.elements(
                Member {
                    raw_type: m.raw_type & !16,
                    ..m
                },
                n,
                depth + 1,
            );
        }
        if m.raw_type & 32 != 0 {
            return self.elements(
                Member {
                    raw_type: m.raw_type & !32,
                    ..m
                },
                m.tuple_size,
                depth + 1,
            );
        }
        Ok(match m.raw_type & 15 {
            0 => Value::Void,
            1 => Value::Byte(self.take(1)?[0]),
            2 => Value::Integer(self.integer()?),
            3 => {
                let b = self.take(4)?;
                Value::Real(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            }
            k @ 4..=7 => Value::Vector(FloatData {
                bytes: self.take([16, 32, 48, 64][k as usize - 4])?,
            }),
            8 => Value::Reference(self.count()?),
            9 => {
                let id = self.class_id(m.class_name.ok_or("struct class missing")?)?;
                self.structs(id, 1, depth + 1, false)?
                    .pop()
                    .ok_or("missing struct")?
            }
            10 => Value::String(self.string()?),
            _ => return Err(Error::UnsupportedType(m.raw_type)),
        })
    }
    fn elements(&mut self, m: Member<'a>, n: usize, depth: usize) -> Result<Value<'a>> {
        self.depth(depth)?;
        if n > self.limits.max_array_length {
            return Err(Error::BudgetExceeded("array length"));
        }
        spend(&mut self.values, n, "array values")?;
        if m.raw_type & 48 != 0 {
            let mut out = reserve(n, &mut self.allocation)?;
            for _ in 0..n {
                out.push(self.value(m, depth + 1)?);
            }
            return Ok(Value::Array(out));
        }
        Ok(match m.raw_type & 15 {
            1 => Value::Bytes(self.take(n)?),
            2 => {
                let width = self.count()?;
                if !matches!(width, 4 | 8) {
                    return Err(Error::InvalidData("integer array width"));
                }
                let mut values = reserve(n, &mut self.allocation)?;
                for _ in 0..n {
                    values.push(self.integer()?);
                }
                Value::Integers {
                    width: width as u8,
                    values,
                }
            }
            3 => Value::Reals(FloatData {
                bytes: self.take(n.checked_mul(4).ok_or("real array overflow")?)?,
            }),
            k @ 4..=7 => {
                let components = if k == 4 {
                    let c = self.count()?;
                    if !matches!(c, 3 | 4) {
                        return Err(Error::InvalidData("vec4 component count"));
                    }
                    c
                } else {
                    [8, 12, 16][k as usize - 5]
                };
                Value::Vectors {
                    components,
                    data: FloatData {
                        bytes: self.take(
                            n.checked_mul(components)
                                .and_then(|v| v.checked_mul(4))
                                .ok_or("vector array overflow")?,
                        )?,
                    },
                }
            }
            9 => {
                let id = self.class_id(m.class_name.ok_or("struct array class missing")?)?;
                Value::Array(self.structs(id, n, depth + 1, true)?)
            }
            _ => {
                let mut out = reserve(n, &mut self.allocation)?;
                for _ in 0..n {
                    out.push(self.value(m, depth + 1)?);
                }
                Value::Array(out)
            }
        })
    }
    fn expanded(&mut self, value: Value<'a>, n: usize) -> Result<Vec<Value<'a>>> {
        if let Value::Array(v) = value {
            if v.len() != n {
                return Err(Error::InvalidData("column array size"));
            }
            return Ok(v);
        }
        let mut out = reserve(n, &mut self.allocation)?;
        match value {
            Value::Bytes(b) => out.extend(b.iter().copied().map(Value::Byte)),
            Value::Integers { values, .. } => out.extend(values.into_iter().map(Value::Integer)),
            Value::Reals(v) => out.extend(v.values().map(Value::Real)),
            Value::Vectors { components, data } => {
                for bytes in data.bytes.chunks_exact(components * 4) {
                    out.push(Value::Vector(FloatData { bytes }));
                }
            }
            _ => return Err(Error::InvalidData("column values")),
        }
        if out.len() != n {
            return Err(Error::InvalidData("column length"));
        }
        Ok(out)
    }
    fn structs(
        &mut self,
        class: usize,
        n: usize,
        depth: usize,
        columnar: bool,
    ) -> Result<Vec<Value<'a>>> {
        self.depth(depth)?;
        if n > self.limits.max_array_length {
            return Err(Error::BudgetExceeded("struct array length"));
        }
        let members = self.flat_members(class)?;
        let flags = self.take(members.len().div_ceil(8))?;
        let present = (0..members.len())
            .filter(|&i| flags[i / 8] >> (i % 8) & 1 != 0)
            .count();
        spend(
            &mut self.values,
            n.checked_mul(present).ok_or("field value count overflow")?,
            "struct fields",
        )?;
        let mut out = reserve(n, &mut self.allocation)?;
        for _ in 0..n {
            out.push(Value::Struct(Object {
                class_index: class,
                fields: reserve(present, &mut self.allocation)?,
            }));
        }
        for (i, m) in members.into_iter().enumerate() {
            if flags[i / 8] >> (i % 8) & 1 == 0 {
                continue;
            }
            if columnar {
                let column = self.elements(m, n, depth + 1)?;
                let column = self.expanded(column, n)?;
                for (row, value) in out.iter_mut().zip(column) {
                    let Value::Struct(o) = row else {
                        return Err(Error::InvalidData("struct row"));
                    };
                    o.fields.push(Field {
                        name: m.name,
                        value,
                    });
                }
            } else {
                if n != 1 {
                    return Err(Error::InvalidData("inline struct count"));
                }
                let value = self.value(m, depth + 1)?;
                let Value::Struct(o) = &mut out[0] else {
                    return Err(Error::InvalidData("inline struct row"));
                };
                o.fields.push(Field {
                    name: m.name,
                    value,
                });
            }
        }
        Ok(out)
    }
}
fn validate_object(
    o: &Object<'_>,
    g: &Tagfile<'_>,
    steps: &mut usize,
    depth: usize,
    max: usize,
) -> Result<()> {
    for f in &o.fields {
        validate_value(&f.value, g, steps, depth + 1, max)?;
    }
    Ok(())
}
fn validate_value(
    v: &Value<'_>,
    g: &Tagfile<'_>,
    steps: &mut usize,
    depth: usize,
    max: usize,
) -> Result<()> {
    if depth >= max {
        return Err(Error::BudgetExceeded("reference validation depth"));
    }
    spend(steps, 1, "reference validation")?;
    match v {
        Value::Reference(id) => {
            g.object(*id)?;
        }
        Value::Struct(o) => validate_object(o, g, steps, depth + 1, max)?,
        Value::Array(a) => {
            for v in a {
                validate_value(v, g, steps, depth + 1, max)?;
            }
        }
        _ => (),
    };
    Ok(())
}
