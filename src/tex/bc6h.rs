//! Signed BC6H block dispatch to the MIT generic bcdec_rs dependency.
//! No FFXIV parsing lives in that dependency; lengths/budgets are checked here
//! by Tex/Mip before fixed-size block calls. See docs/provenance/bc6h.md.
use super::{Surface, half_to_f32, quantize};

fn pixels(surface: Surface<'_>, mut emit: impl FnMut(usize, [f32; 4])) {
    let width = usize::from(surface.width);
    let height = usize::from(surface.height);
    let columns = width.div_ceil(4);
    for (index, block) in surface.data.chunks_exact(16).enumerate() {
        let mut halfs = [0u16; 48];
        // These are fixed valid buffer lengths and a pitch in u16 elements.
        bcdec_rs::bc6h_half(block, &mut halfs, 12, true);
        let bx = (index % columns) * 4;
        let by = (index / columns) * 4;
        for y in 0..4.min(height - by) {
            for x in 0..4.min(width - bx) {
                let p = (y * 4 + x) * 3;
                emit(
                    (by + y) * width + bx + x,
                    [
                        half_to_f32(halfs[p]),
                        half_to_f32(halfs[p + 1]),
                        half_to_f32(halfs[p + 2]),
                        1.,
                    ],
                );
            }
        }
    }
}

pub(super) fn decode_float(surface: Surface<'_>, output: &mut [f32]) {
    pixels(surface, |i, rgba| {
        output[i * 4..i * 4 + 4].copy_from_slice(&rgba)
    });
}

pub(super) fn decode_bytes(surface: Surface<'_>, output: &mut [u8]) {
    pixels(surface, |i, rgba| {
        output[i * 4..i * 4 + 4].copy_from_slice(&rgba.map(quantize))
    });
}
