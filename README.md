# icexiv-data

A Rust library for reading FFXIV data, with SqPack extraction, EXH/EXD parsing,
and CPU decoding of extracted 2D TEX textures.

## Usage

Read a resource from a local installation with the `local` feature:

```rust
use icexiv_data::sqpack::{SqPack, local::Local};

async fn read_header() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source = Local::new("/path/to/game/sqpack").await?;
    let bytes = SqPack::new(source).read("exd/item.exh").await?;
    Ok(bytes)
}
```

`read` returns standard binary resources and texture resources; texture results are
TEX files ready for the decoder below. For bulk lookups, enable the index cache:

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

The TEX decoder supports the common integer formats, BC1–BC5, BC7 and five float
layouts, with checked mip ranges and an explicit output byte budget. BC6H,
swizzled/tiled layouts, arrays, cubes and volumes return explicit errors. PNG
encoding is left to callers. SqPack extraction supports type-2 standard entries
and type-4 texture entries; see [TEX documentation](docs/tex/README.md) for
supported layouts and [SqPack provenance](docs/sqpack/provenance.md) for sources.

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
