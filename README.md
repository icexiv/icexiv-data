# icexiv-data

A Rust library for reading FFXIV data, with SqPack extraction and EXH/EXD parsing.

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

See [examples/](examples/) for runnable examples and their usage instructions,
and [docs/](docs/README.md) for format documentation.

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
