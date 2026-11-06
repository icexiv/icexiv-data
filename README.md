# icexiv-data

A Rust library for reading FFXIV data, with SqPack extraction, EXH/EXD parsing,
and CPU decoding of extracted TEX textures.

See the [format support overview](docs/support.md) for parser/decoder variants,
usable outputs, validation evidence and limitations across the library.

IMC default/variant records are available through `imc::Imc`. Parsing borrows
the complete input with no allocation, validates exact table length and returns
errors for missing subsets or parts. Part arguments are file mask bit numbers;
no path resolution or equipment-slot mapping is performed. Raw flags and the
animation byte remain available alongside bitfield accessors.
See [rendering requirements coverage](docs/requirements-coverage.md) and
[IMC sample evidence](docs/provenance/imc.md) for validation limits.

## Excel tables

`excel::Exh` / `excel::Exd` decode EXH v3 and EXD v2 metadata, ordinary rows,
parent/subrow keys and all declared scalar/packed-bool/string fields by numeric
column index. Subrows share one parent string pool after all fixed records;
strings remain borrowed raw bytes, without UTF-8 or SeString interpretation.
64-bit values retain all bits on native and WASM. `parse_with_limits` bounds
input, metadata, columns, parent/output row counts and validation work; lazy
string reads retain the page's explicit length cap. EXD data-section lengths,
row spans, duplicate IDs and overlapping records are checked. Page ranges are ID
intervals, not populated-row counts. External schemas supply business column names.
See [Excel evidence](docs/provenance/excel.md) for actual catalog/model/subrow tables.
The `show_root_exl` example lists the EXL manifest by sheet path; it preserves
numeric suffixes in sheet names instead of mistaking them for page numbers.

## Equipment declarations

`eqdp::Eqdp` provides borrowed sparse-block lookup for Win32 `0x0101` equipment
and accessory tables. It distinguishes implicit zero blocks from invalid entry
indices and retains raw slot flags and unknown bits. `equipment_slot` and
`accessory_slot` expose named domain-specific slots, including separate left/right
rings; callers supply the domain from the resource path. Material/model flag
meanings have static resource evidence, not complete runtime fallback semantics.
See [EQDP evidence](docs/provenance/eqdp.md). No path routing is performed.

`eqp::Eqp` reads compressed Win32 display-parameter blocks and exposes raw words,
five slot regions and named declaration bits. Entry zero is the bitmap, not gear
data; absent blocks return `None`, without a guessed runtime default. No combined
character visibility is computed. `uses_*_parameters` exposes region-source
declarations; coverage and culling accessors preserve the coupled two-bit fields.
`HeadFlags::hair_coverage_code()` retains the coupled three-bit hair code (0..7);
the individual hair bit accessors do not independently resolve visibility.
Tail getters preserve the separate body/leg declarations without applying their
enabled gates. Ear/hat getters and `Flags::bit(59)` likewise retain file fields;
they do not evaluate the shared tail/zear slot or character-state conditions.
Public flag names and statically observed client consumption are distinguished
in [EQP evidence](docs/provenance/eqp.md).

## MDL geometry

`mdl::Mdl` reads extracted Win32 version `0x01000006` without I/O or metadata
allocation. It exposes LOD roles, material/attribute/bone names, submeshes,
variable bone palettes, ElementId records, bounds, shape replacements and
declaration-driven vertex/index access:

```rust
use icexiv_data::mdl::Mdl;

fn first_position(bytes: &[u8]) -> Result<Option<[f32; 8]>, icexiv_data::mdl::Error> {
    let model = Mdl::parse(bytes)?;
    model.geometry(0, 0)?.attribute(0, 0, 0)
}
```

Scalar types 0–3 (float1–float4), 5/17 (byte4/byte8), 8 (normalized byte4) and 13/14
(half) decode to eight lanes with unused lanes zero. Binary32 inputs preserve
all stored bits, including signed zero and NaN payloads. No coordinate, UV, gamma
or shader transformations are applied. `skin_influences` divides byte weights
by 255 and retains palette-local indices, up to eight influences. `shape_delta`
uses the source index occurrence without baking shapes or splitting shared
vertices. Unknown runtime extensions and raw flags remain accessible.
`Geometry::attribute` distinguishes each `(usage, usage_index)` and rejects
duplicate matches instead of choosing a declaration by order. `skin_influences`
requires a unique usage-index-zero weight/index pair: both absent returns
`None`, but one missing member returns an error. Both queries reject invalid
vertex indices even when the requested input is absent. Raw declarations and
`vertex_element` remain available for explicit caller selection.
`attribute_bit(name, work_budget)` resolves a raw name to this MDL's u32 mask
bit. Absence returns `None`; duplicate names, an unrepresentable index or exhausted
work budget return errors. It does not evaluate EQP rules or character visibility.
`terrain_shadow_geometry` borrows the LOD's separate terrain mesh vertex/index
buffers. `position_half4` decodes the verified eight-byte terrain vertex layout,
retaining its fourth lane; other strides remain accessible as raw records and
return an error for half4 decoding. No terrain rendering or projection is applied.

`parse_with_limits` bounds input, records, index validation and total validation
work; `decode_attribute` additionally takes an output byte budget. Unknown scalar
types remain metadata but return an error when decoded. Other MDL versions
return an explicit error. See [MDL evidence](docs/provenance/mdl.md) for sample
coverage and unresolved declaration, rotation and runtime-extension semantics.

## Shared scene assets

`sgb::Sgb` reads the verified Win32 SGB1/SCN1 furniture profile (one scene and
one layer group), preserving layers, instance IDs, names and original nine-lane
TRS. Checked relative offset tables resolve BG model/collision and SharedGroup
paths; `dependencies()` retains each nonempty occurrence and its owning instance.
It does not guess assets from arbitrary strings or filenames. Unsupported
scene/group profiles return errors. Other instance types retain their common
headers and are enumerable, rather than establishing complete asset absence.
Input, metadata allocation, layer/instance counts, string length and work are
budgeted; no dependency I/O, recursion or transform composition is performed.
See [SGB evidence](docs/provenance/sgb.md).

## VFX assets and textures

`avfx::Avfx` provides checked borrowed global blocks, six node kinds, texture
paths and embedded draw/emission geometry. Explicit field views retain unknown
blocks and type-specific Data without guessing recursive structure. Scheduler
and timeline helpers decode the final cumulative Item records; `Curve` exposes
signed-time keyframes and `Clip` exposes its fixed record. Positions/UV halfs,
biased normal/tangent bytes, colors and indices are decoded without simulation.
Input, block count, allocation and validation work are bounded. Type-specific
Data views classify 31 verified emitter/particle/effector/binder combinations,
decode their integer/byte/float parameters, model lists and curve blocks, and
retain unknown fields/types. `emitter_items` selects the last appended record of
each ItEm/ItPr block and uses its outer tag for the target kind, preserving
particle-first interleaving; texture slots preserve static, mask-list and
animated selection. `asset_report` records signed reference values and their raw
table ranges, including disabled slots; negative values are not confirmed absence
and out-of-range values are not unconditional execution failures. All these views
have explicit block/item/output/work budgets. Other Data variants and runtime
reference policies remain outside the verified subset.
`binder_properties` preserves optional Start/First/Second/Goal slots, BPT/BPTP,
BPID, raw names, ring values and position groups without choosing a host or
defaulting missing values. BPID is not a skeleton bone-array index. Static
owner ElementId/parent-bone correspondences are documented in
[weapon binder evidence](docs/provenance/binder.md); no pose is composed.

AVFX `.atex` bytes go directly to `tex::Tex::parse`, including all declared mips,
image views and float-output support; SqPack type-2 extraction is valid for ATEX.
See [VFX evidence](docs/provenance/vfx.md) for the verified asset subset.

## Skeletons and animation

`pap::Pap` reads Win32 `0x00020001` names, container-order animation indices and
binding references, retaining unknown record/header bytes and timeline data.
Container-level Havok alignment is preserved in `havok_padding`: either no
padding, or up to 15 zero bytes ending at a file-relative 16-byte boundary.
Standalone Havok tagfile parsing rejects trailing bytes.
`tagfile` is optional: a zero-record package can contain only alignment bytes
and no Havok stream. Empty packages do not produce a default animation.
`Tagfile::spline_animation` decodes v0 transform spline controls with allocation,
track/block/control/work budgets. Vector u8/u16 and quaternion polar32, 40-bit,
48-bit and uncompressed encodings are handled; unknown encodings and nonzero
float-track counts return errors, never identity placeholders. Samples use finite
milliseconds clamped to the animation duration and explicit output/work budgets,
returning track-space translation, normalized xyzw rotation and scale. Binding
indices and raw blend hints remain separate; no additive composition or skeleton
assembly is performed. See [PAP evidence](docs/provenance/animation.md).
Large borrowed animation byte arrays may require an explicit `max_array_length`
in `Pap::parse_with_limits`; the default 1,000,000-element budget is not raised
automatically. Allocation, value, spline-control and sampling budgets still apply.

`sklb::Sklb` reads Win32 1200/1300 containers and parses the complete embedded
Havok binary v3 object graph. `skeletons(output_byte_budget)` decodes every
remembered hkaSkeleton v3 with checked parent forests, raw names and local rest
translation/quaternion/scale lanes. Quaternion order is xyzw; all twelve lanes
are retained without normalization or axis conversion. `animation_skeleton_ids`
on the tagfile distinguishes container-selected skeletons from mapper-side ones.
Input, schema, strings, objects, arrays, allocation, nesting and validation work
are budgeted; configurable nesting is capped at 128. Unknown variants return explicit errors; auxiliary bytes and generic
graph fields remain available. See [skeleton evidence](docs/provenance/skeleton.md).

## Body parameters and bone deformation

`cmp::Cmp` borrows the Win32 eight-race `human.cmp` profile: raw RGBA palettes
and height/tail/bust range endpoints indexed by zero-based race/clan and body
type 1–5. It validates the exact profile size and finite, ordered ranges;
other color-prefix lengths return explicit errors. `Range::at_percent` and
`VectorRange::at_percent` evaluate mathematical percentages `0..=100`, rejecting
other inputs with `InvalidPercentage`. Weighted f64 intermediates avoid f32
difference overflow; endpoint bits are preserved at 0 and 100. This is not a
customize sentinel/fallback policy or a promise of native instruction-level
rounding. No pose scaling is performed. See [CMP evidence](docs/provenance/cmp.md).

`pbd::Pbd` borrows race-deformer records, a checked connection forest and actual
bone-name/3×4 affine matrix data. It also reads version-1 EPBD/PACK extensions,
including embedded PBDs under shared record, name, work and nesting budgets.
`Matrix::column_major_4x4` preserves coefficients without TRS decomposition.
`Matrix::compose(rhs)` is ordinary affine multiplication `self * rhs` for column
vectors (`rhs` acts first), preserving shear. It does not select a deformer chain,
resolve EPBD conditions or choose source/target skeletons.
`Matrix::try_inverse()` provides fixed-size general affine math, retaining shear.
It uses f64 partial-pivot elimination, rejects nonfinite input, zero computed
pivots and nonfinite f32 output, and returns a distinct `MatrixInverseError`.
It applies no epsilon/conditioning policy and does not interpret reverse race
pairs or construct inverse bind matrices from rest poses.
`ancestor_path(descendant_index, ancestor_index, max_steps)` returns a borrowed,
allocation-free iterator of deformer table indices towards (excluding) the
ancestor. Equal indices yield an empty path; unrelated nodes return `None`.
Invalid indices and exhausted record-read budgets return errors. The query
accepts table indices, not skeleton IDs or connection indices, and preflights
the budget for traversal and iteration. It does not apply the matrices.
`Extended::version_tag()` and `unknown_root_scalar()` preserve the optional u16
root fields without treating them as offsets or inventing omitted values.
Root tags other than 1 remain visible as structural data, not a promise of native
acceptance. `ExtendedEntry::local_skeleton_ids()` exposes the low u16 of each raw
u32 selector in the verified Win64 profile; `conditions()` retains all 32 bits.
`Extended::find_entry(child, parent, max_entries)` queries an exact raw u32 pair
with an entry-read budget. Duplicate pairs are an explicit query ambiguity;
neither budget exhaustion nor an early match is treated as confirmed absence or
unique success. It performs no native key coercion or selector/fallback policy.
Unknown scale and EPBD selectors retain their raw data; the library does not infer
runtime selector choice or bake race deformations. Unsupported PACK variants
and unrecognized nonzero tails are explicit errors. See [PBD evidence](docs/provenance/pbd.md).

## Materials and shader metadata

`mtrl::Mtrl` borrows version `0x01030000` material bytes and exposes texture/UV/
color-set references, shader keys, constants, samplers and raw extra data.
`color_table()` distinguishes an absent dataset from an unsupported dataset
layout. Legacy 16-row and Dawntrail 32-row tables decode every half lane without
clamping; dye rows retain the raw bits with template, override-mask and channel
accessors. `inline_color_table_declaration()` exposes the full additional-data
u32, half-lane/row dimensions and dye flag when an inline declaration exists;
`table_flags()` retains its low-byte view. `color_table()` rejects unsupported
declared dimensions and dye/size conflicts instead of choosing a layout by byte
length alone. Raw datasets remain accessible. Shader-specific formulas and
material routing belong to callers.

`DyeRow::canonical_override_lane_mask(DyeOverrideProfile)` projects declaration
flags onto eligible lanes of a 32-half intermediate row. The profile is explicit:
it is not selected from `ColorLayout` or the file's row count. Legacy source colors
are not converted by this API, so its mask is not a Legacy file-row mask. Channel
selection, STM lookup and conditional writes are separate from eligibility.

`mtrl::dye::apply_selected_row` resolves the dye's template in an explicitly
provided STM and applies eligible replacements to a caller-supplied `[u16; 32]`
intermediate row. It returns an owned half-bit row and actual written-lane mask,
preserving unselected bits, NaN payloads, HDR and signed zero. Selected all-zero
specular RGB retains its source; other zero replacements are written.
`apply_channel_row` additionally gates a Dawntrail dye word by its encoded two-bit
channel (`Ok(None)` on mismatch). Legacy words require explicit row selection.
The STM layout must match the chosen profile, independently of source layout.
Missing template IDs and invalid one-based stains are explicit errors; even an
empty declaration mask requires valid template/stain inputs for these row helpers.

`canonical_inline_color_table()` produces an immutable 32×32-half base from an
inline declaration; an absent declaration returns `None`, while a declared table
without its dataset is an error. `ColorTable::to_canonical()` also accepts an
explicitly selected dataset. Legacy rows undergo the documented lane conversion,
and rows 16–31 copy converted row zero without inheriting its dye record.
`ColorRow::canonical_half_bits()` exposes the single-row conversion. Lane 7 uses
binary32 nearest-even subtraction followed by binary16 truncation; fixed-point
evaluation makes it independent of host rounding controls. Copied NaN payloads,
signed zero, negatives and HDR survive; the computed lane has explicit NaN/zero
semantics in its API documentation.

`dye::apply_table(&base, profile, templates, [stain0, stain1])` returns all 2048
color bytes plus per-row write masks. Each evaluation starts from the immutable
base. Stain 0 resets its rows; Legacy dye records select slot 0, and encoded
channels 2/3 match neither request. A confirmed absent template retains the base
row and sets `missing_template_rows`; a missing STM or decode failure is an error.
Stains 254/255 are rejected. `None` for the STM is valid when no row requests an
override. `DyeOverrideProfile::for_shader_package(mtrl.shader_package()?)` selects
Legacy for `characterlegacy.shpk` and Dawntrail otherwise. Supply
`stainingtemplate.stm` for Legacy or `stainingtemplate_gud.stm` for Dawntrail.
Both base and result expose half bits, little-endian bytes and unclamped f32 values.
See [complete table evidence](docs/provenance/legacy-color-conversion.md#完整色表输出与染色关联).

`shpk::Shpk` reads Win32 DX11 versions `0x0d01`/`0x0e01`: global/per-shader resources and
names, default material parameters and keys, shader byte ranges, nodes, passes
and aliases. It does not execute shaders; node-alias clusters remain raw.
`version()` distinguishes the 84-byte and 88-byte headers;
`declared_cluster_count()` returns `None` for the absent 0x0d01 field, while
`cluster_count()` returns its logical count of zero.
`parse_with_limits` bounds input bytes, records, metadata allocation and name
validation. Resource IDs can be checked with `shpk::name_id`, whose CRC differs
from SqPack path hashing. Missing default parameter bytes return `None`, rather
than fabricated zeros. No renderer or material-family model is required.

Both modules preserve names as raw bytes and reject invalid lengths and
references. Raw constant bytes are available even when float interpretation
returns an alignment error. See [material validation evidence](docs/provenance/materials.md)
for covered variants and the Legacy dye evidence gap.

`stm::Stm` borrows Legacy `0x0101` and Dawntrail `0x0201` staining templates.
Columns expose zero, uniform, dense and indexed half-float values without clamping.
Indexed columns carry stain IDs 1–253; their marker byte is not a stain value.
Input size, template count and validation work are budgeted. Template lookup does
not select a file from the MTRL color-table layout: a 32-row material can refer
to a Legacy template. See [STM evidence](docs/provenance/stm.md).

## Resource access

Read a resource from a local installation with the `local` feature:

```rust
use icexiv_data::sqpack::{SqPack, local::Local};

async fn read_header() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source = Local::new("/path/to/game/sqpack").await?;
    let bytes = SqPack::new(source).read("exd/item.exh").await?;
    Ok(bytes)
}
```

`read` returns standard binary resources, reconstructed MDL models and TEX
textures. For bulk lookups, enable the index cache:

```rust
# use icexiv_data::sqpack::{SqPack, local::Local};
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let source = Local::new("/path/to/game/sqpack").await?;
let sqpack = SqPack::new(source)
    .with_index_cache(Some(64 * 1024 * 1024))
    .with_max_read_bytes(Some(64 * 1024 * 1024));
let icon = sqpack.read("ui/icon/062000/062101_hr1.tex").await?;
# let _ = icon;
# Ok(())
# }
```

See [examples/](examples/) for runnable examples and their usage instructions,
and [docs/](docs/README.md) for format documentation.

Decode an already extracted TEX file without I/O or GPU dependencies:

```rust
use icexiv_data::tex::Tex;

fn decode_icon(bytes: &[u8]) -> Result<Vec<u8>, icexiv_data::tex::Error> {
    Tex::parse(bytes)?.mip(0)?.decode_rgba8(16 * 1024 * 1024)
}
```

The TEX decoder supports the common integer formats, BC1–BC7 (BC6H signed SF16)
and seven uncompressed float layouts (including R16F and packed R11G11B10F), with checked mip ranges and an explicit
output byte budget. Swizzled/tiled layouts return explicit errors. Linear 2D arrays, six-face cubes
and volumes expose every image through `Mip::image(layer, face, z)`.
`Mip::cube_face(CubeFace)` names cube faces in the Win32 Direct3D convention
(`+X,-X,+Y,-Y,+Z,-Z`) without changing pixel row order. Other layouts, including
six-layer 2D arrays, return `NotCubeTexture`; no world-axis or UV conversion is
implied. Whole-mip decoding concatenates all images and budgets the entire output. `decode_rgba32f`
retains unclamped float channels, including HDR and non-finite values.
One-channel floats replicate across RGB; packed R11G11B10F expands RGB with A=1.
Half/packed NaNs retain their classification, without a payload/sign guarantee.
`Tex::parse_packed_volume` is an explicit volume profile for headers whose active
offset words all equal 80: it borrows consecutive dimension-derived mip spans,
requires the full payload, and never rewrites the source header or fills gaps.
`declared_mip_offsets()` retains the active source words and
`uses_packed_volume_layout()` identifies this interpretation. `Tex::parse`
keeps strict declared-offset checks; other repeated/invalid patterns are not
silently accepted as packed data.

`tex::Header::parse` validates and preserves the first 80 bytes independently of
pixel storage. `mip_range(index)` gives the complete encoded span in the logical
file; `encoded_len()` includes the header and any declared gaps through the last
mip. `header.mip(index, payload)` borrows exactly one complete mip, rejecting both
short and oversized payloads with `MipSizeMismatch`. It includes every layer,
face and depth slice and exposes the same image/float decoding as a whole TEX.
`Mip::source_index()` and `source_mip_count()` retain the original mip identity.
`Header::parse_packed_volume` selects the repeated-offset profile explicitly.

For type-4 resources, `SqPack::read_texture_group(path, group)` and
`read_texture_group_entry(dat_name, offset, group)` return a `TextureGroup` with
the unchanged prefix, decoded group bytes and original group count/file length.
All group/span tables and ordered physical ranges are checked; only the selected
group's block headers and compressed streams are read and decoded. The read cap
bounds each physical read and the prefix plus selected decoded bytes. A group
with zero declared decoded length is bounded by the logical space left after the
other groups' nonzero declarations.

`group.volume_mip()` binds the one-group-per-mip volume profile: an 80-byte header,
matching group/mip counts, consecutive logical mip spans (or repeated offset 80),
matching full logical length, and a complete selected payload. Groups in other
layouts need their own mapping to mips. For a complete base mip:

```rust
use icexiv_data::sqpack::{SqPack, local::Local};

async fn decode_volume_base() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source = Local::new("/path/to/game/sqpack").await?;
    let pack = SqPack::new(source).with_max_read_bytes(Some(16 * 1024 * 1024));
    let group = pack
        .read_texture_group("bgcommon/nature/sky/texture/sky_441.tex", 0)
        .await?;
    let mip = group.volume_mip()?;
    Ok(mip.decode_rgba8(16 * 1024 * 1024)?)
}
```

BC6H returns signed RGB values with alpha 1; its generic MIT decoder dependency
and independent numerical evidence are recorded in [BC6H provenance](docs/provenance/bc6h.md).
PNG encoding is left to callers. SqPack extraction supports type-2 standard entries,
type-3 model entries and type-4 texture entries. Model reconstruction uses actual
decoded segment lengths (not aligned capacities), preserves all LOD buffers, and
checks segment/block ranges and the output budget. Index2 collision lookup uses
the complete lowercased path and the auxiliary name table. `Index::find_path`
also exposes location-only queries. Container parsing retains raw collision
records (including sentinel/unknown/trailing bytes); auxiliary tables default
to a 16 MiB cap, configurable for borrowed parsing with
`parse_with_collision_budget`. Async reads also obey the configured per-read cap,
and the index cache accounts for both tables. Main-table-only views cannot
resolve flagged entries; missing names return no match, duplicate matching paths
report ambiguity. Primary-index synonym tables and container SHA-1 verification
are not interpreted. See [index2 evidence](docs/provenance/sqpack-index2.md).
Whole-entry `read`/`read_entry` reject unaccounted type-4 output bytes. The 273
short volume entries have complete base mips and 672 complete individual mips;
selected-group decoding makes those pixels available while incomplete mip
requests return length errors. Full six-level reconstruction retains its
output-size error. See [selected-mip evidence](docs/provenance/volume-mips.md).
See the `tex` API documentation for supported decoder layouts and
[model reconstruction evidence](docs/provenance/sqpack-model.md) for sample scope.

## Development

```sh
nix develop
just check-all
```

Build the documentation with `notist build`.

## Acknowledgements

- [XIV Dev Wiki](https://github.com/xivdev/docs)
- [quilan1/ffxiv-parser-lib](https://github.com/quilan1/ffxiv-parser-lib)
- [yeliex/sqpack](https://github.com/yeliex/sqpack)
- [Lumina](https://github.com/NotAdam/Lumina) (TEX format declarations; WTFPL v2)
- [DirectXTex](https://github.com/microsoft/DirectXTex) (BC7 tables; MIT)
- [VFXEditor](https://github.com/0ceal0t/Dalamud-VFXEditor) (material/shader field review; MIT; source review details in the provenance documents)
