use super::{AssetLimits, Block, Error, Fields, Kind, Node, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinderSlot {
    Start,
    First,
    Second,
    Goal,
}
#[derive(Clone, Copy, Debug)]
pub struct RingParameters {
    pub enabled: Option<i32>,
    pub progress_time: Option<i32>,
    pub position: [Option<f32>; 3],
    pub radius: Option<f32>,
}
#[derive(Clone, Copy, Debug)]
pub struct BinderProperty<'a> {
    pub slot: BinderSlot,
    pub block: Block<'a>,
    pub fields: Fields<'a>,
    pub bind_point: Option<i32>,
    pub target_point: Option<i32>,
    pub bind_point_id: Option<i32>,
    /// Raw NUL-terminated name, without padding. The token `null` is retained
    /// literally, not treated as absence. An encoded empty block is Some(empty).
    pub name: Option<&'a [u8]>,
    pub generate_delay: Option<i32>,
    pub coordinate_update_frame: Option<i32>,
    pub coordinate_type: Option<i32>,
    pub ring: RingParameters,
    pub position: Option<Fields<'a>>,
}
impl<'a> Node<'a> {
    /// Decode property slots as asset metadata. Does not choose a caster/target,
    /// supply missing-slot defaults, resolve BPID globally or assemble a pose.
    pub fn binder_properties(self, limits: AssetLimits) -> Result<Vec<BinderProperty<'a>>> {
        if self.kind != Kind::Binder {
            return Err("not a binder".into());
        }
        let mut work = limits.max_work;
        let mut blocks = limits.max_blocks;
        let mut bytes = limits.max_output_bytes;
        let spend = |budget: &mut usize, n: usize| -> Result<()> {
            *budget = budget.checked_sub(n).ok_or(Error::BudgetExceeded)?;
            Ok(())
        };
        spend(&mut work, self.fields.len())?;
        let classify = |b: Block<'_>| {
            if b.is(b"PrpS") {
                Some((0, BinderSlot::Start))
            } else if b.is(b"Prp1") {
                Some((1, BinderSlot::First))
            } else if b.is(b"Prp2") {
                Some((2, BinderSlot::Second))
            } else if b.is(b"PrpG") {
                Some((3, BinderSlot::Goal))
            } else {
                None
            }
        };
        let count = self
            .fields
            .iter()
            .filter(|b| classify(*b).is_some())
            .count();
        if count > limits.max_items {
            return Err(Error::BudgetExceeded);
        }
        spend(
            &mut bytes,
            count
                .checked_mul(std::mem::size_of::<BinderProperty<'_>>())
                .ok_or("binder allocation overflow")?,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        let mut seen = 0u8;
        for block in self.fields.iter() {
            let Some((index, slot)) = classify(block) else {
                continue;
            };
            if seen & (1 << index) != 0 {
                return Err("duplicate binder property slot".into());
            }
            seen |= 1 << index;
            let fields = block.fields(blocks)?;
            spend(&mut blocks, fields.len())?;
            spend(
                &mut work,
                fields.len().checked_mul(18).ok_or("binder work overflow")?,
            )?;
            let name = if let Some(b) = fields.one(b"Name")? {
                spend(
                    &mut work,
                    b.payload
                        .len()
                        .checked_mul(2)
                        .ok_or("binder name work overflow")?,
                )?;
                Some(if b.payload.is_empty() {
                    &[][..]
                } else {
                    b.string()?
                })
            } else {
                None
            };
            let position = if let Some(b) = fields.one(b"Pos")? {
                let f = b.fields(blocks)?;
                spend(&mut blocks, f.len())?;
                spend(&mut work, f.len())?;
                Some(f)
            } else {
                None
            };
            output.push(BinderProperty {
                slot,
                block,
                fields,
                bind_point: fields.integer(b"BPT")?,
                target_point: fields.integer(b"BPTP")?,
                bind_point_id: fields.integer(b"BPID")?,
                name,
                generate_delay: fields.integer(b"GenD")?,
                coordinate_update_frame: fields.integer(b"CoUF")?,
                coordinate_type: fields.integer(b"BCT")?,
                position,
                ring: RingParameters {
                    enabled: fields.integer(b"bRng")?,
                    progress_time: fields.integer(b"RnPT")?,
                    position: [
                        fields.real(b"RnPX")?,
                        fields.real(b"RnPY")?,
                        fields.real(b"RnPZ")?,
                    ],
                    radius: fields.real(b"RnRd")?,
                },
            });
        }
        Ok(output)
    }
}
