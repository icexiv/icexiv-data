//! BC1/2/3/4/5 decoding from the documented block layouts. Inputs and output
//! lengths have already been checked by the TEX parser and decode budget.

use super::{Format, Mip, expand5};

pub(super) fn decode(mip: Mip<'_>, output: &mut [u8]) {
    let width = usize::from(mip.width);
    let height = usize::from(mip.height);
    let columns = width.div_ceil(4);
    let block_size = match mip.format {
        Format::Bc1 | Format::Bc4 => 8,
        _ => 16,
    };
    for (block_index, block) in mip.data.chunks_exact(block_size).enumerate() {
        let pixels = match mip.format {
            Format::Bc1 | Format::Bc2 | Format::Bc3 => color_block(mip.format, block),
            Format::Bc4 => bc4_block(block).map(|value| [value, value, value, 255]),
            Format::Bc5 => {
                let red = bc4_block(&block[..8]);
                let green = bc4_block(&block[8..]);
                std::array::from_fn(|i| [red[i], green[i], 0, 255])
            }
            _ => unreachable!("only block-compressed formats reach this decoder"),
        };
        let origin_x = (block_index % columns) * 4;
        let origin_y = (block_index / columns) * 4;
        for (i, rgba) in pixels.into_iter().enumerate() {
            let x = origin_x + i % 4;
            let y = origin_y + i / 4;
            if x >= width || y >= height {
                continue;
            }
            let offset = (y * width + x) * 4;
            output[offset..offset + 4].copy_from_slice(&rgba);
        }
    }
}

fn color_block(format: Format, block: &[u8]) -> [[u8; 4]; 16] {
    let color = &block[block.len() - 8..];
    let first = u16::from_le_bytes([color[0], color[1]]);
    let second = u16::from_le_bytes([color[2], color[3]]);
    let mut palette = [rgb565(first), rgb565(second), [0; 4], [0; 4]];
    if format != Format::Bc1 || first > second {
        palette[2] = mix(palette[0], palette[1], 2, 1, 3);
        palette[3] = mix(palette[0], palette[1], 1, 2, 3);
    } else {
        palette[2] = mix(palette[0], palette[1], 1, 1, 2);
        // palette[3] is transparent black (BC1 three-color mode only).
    }
    let indices = u32::from_le_bytes(color[4..8].try_into().unwrap());
    let alphas = match format {
        Format::Bc2 => {
            let bits = u64::from_le_bytes(block[..8].try_into().unwrap());
            std::array::from_fn(|i| ((bits >> (4 * i)) & 15) as u8 * 17)
        }
        Format::Bc3 => bc4_block(&block[..8]),
        _ => [255; 16],
    };
    std::array::from_fn(|i| {
        let mut rgba = palette[((indices >> (2 * i)) & 3) as usize];
        if format != Format::Bc1 {
            rgba[3] = alphas[i];
        }
        rgba
    })
}

/// Single-channel block shared by BC3's alpha, BC4 and BC5.
fn bc4_block(block: &[u8]) -> [u8; 16] {
    let a = u16::from(block[0]);
    let b = u16::from(block[1]);
    let mut palette = [block[0], block[1], 0, 0, 0, 0, 0, 255];
    let (steps, divisor) = if a > b { (6, 7) } else { (4, 5) };
    for i in 0..steps {
        palette[usize::from(i) + 2] = (((divisor - 1 - i) * a + (i + 1) * b) / divisor) as u8;
    }
    let mut packed = [0u8; 8];
    packed[..6].copy_from_slice(&block[2..8]);
    let bits = u64::from_le_bytes(packed);
    std::array::from_fn(|i| palette[((bits >> (3 * i)) & 7) as usize])
}

fn rgb565(value: u16) -> [u8; 4] {
    let green = (value >> 5) & 63;
    [
        expand5(value >> 11),
        ((green << 2) | (green >> 4)) as u8,
        expand5(value & 31),
        255,
    ]
}

fn mix(a: [u8; 4], b: [u8; 4], wa: u16, wb: u16, divisor: u16) -> [u8; 4] {
    let mut result = [0, 0, 0, 255];
    for channel in 0..3 {
        result[channel] =
            ((u16::from(a[channel]) * wa + u16::from(b[channel]) * wb) / divisor) as u8;
    }
    result
}
