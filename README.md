# icexiv-data

A fresh start for a backend-independent FFXIV data library.

The active crate provides backend-independent SqPack resource extraction with
optional local and HTTP backends. The default build uses pure-Rust DEFLATE;
filesystem, HTTP and runtime dependencies remain opt-in. There is no standalone
CLI. The APIs are not constrained by the previous implementation.

## SqPack access

`icexiv_data::sqpack` uses relative container names (`&str`) and an asynchronous
backend (`PackSource` / `ReadAt`). No file list or registration is required. A
backend may read local files, HTTP ranges or memory. The core performs no I/O
and requires no async runtime; built-in backends are opt-in. The core traits do
not require `Send` futures, allowing browser-local WASM implementations.

```rust,no_run
use icexiv_data::sqpack::{Error, PackSource, SqPack};

async fn read_container_header<S: PackSource>(
    source: S,
    file: &str,
) -> Result<Vec<u8>, Error<S::Error>> {
    let sqpack = SqPack::new(source);
    let reader = sqpack.open(file).await?;
    reader.read_at(0, 16).await
}
```

For game resources, use `sqpack.read("exd/item.exh").await?`, which resolves the
path through an index and extracts the resource bytes. Supported: Win32
little-endian version-1 `.index`/`.index2` and type-2 standard binary entries,
with raw or raw-DEFLATE blocks. Model/texture/placeholder content types and
flagged or ambiguous hash entries return explicit errors, not guessed data.
EXH/EXD sheet parsing is separate and not implemented yet.
Parsing and decompression are synchronous CPU work on the polling thread;
async covers byte acquisition, not automatic background CPU scheduling.
Fixed binary layouts use `binrw` on in-memory cursors; JAMCRC uses `crc32fast`.
Range, size, budget and decompression checks remain explicit rather than being
delegated to unbounded derive-driven collection parsing.

`SqPack::new` performs no I/O. `open` remains the low-level **container file**
access API. The source determines whether a container exists at access time.
Confirmed container absence becomes `MissingFile`; a resource lookup miss
becomes `MissingResource`. Permission, network and decode failures remain errors.
No directory scan, manifest or HTTP directory-listing endpoint is required.
`SqPack::open` and built-in backends validate names before I/O: slash-separated
ASCII letters, digits, `.`, `_` and `-`, with no empty, `.` or `..` components.
These are relative container names, not absolute paths, URLs or resource paths
inside a container.

Reads are checked for arithmetic overflow, file bounds and exact response length.
Confirmed file absence is separate from backend errors. `SqPack::new(source)`
defaults to no per-read byte cap (`None`). Opt in with
`SqPack::new(source).with_max_read_bytes(Some(bytes))`; `Some(0)` allows only
empty reads, and `with_max_read_bytes(None)` removes a cap. This is an optional
`Option<usize>` setting, not a separate configuration type or backend setting.
Readers capture the setting when opened. `read(path)` also checks the declared
extracted size against the cap before allocating or decompressing the resource.
Without a cap, a valid read can request a large allocation; set one when lengths
are untrusted. The cap does not bound
aggregate concurrent allocations. Backends
must enforce bounded response handling before allocation, preserve file-version
consistency across reads, and report permission/transport/version failures as
errors, not absence. Logical-name validation does not replace a filesystem
adapter's symlink/root security policy or an HTTP adapter's response validation.

### Built-in backends

- `local` feature: `sqpack::local::Local` (native Unix/Windows). All filesystem
  work, including opening and metadata checks, runs in Tokio's blocking pool.
  Requires a Tokio runtime; no `RefCell`, shared-cursor seeking or locks.
- `web` feature: `sqpack::web::Web` (native HTTP and browser WASM). Uses reqwest
  and bounded streaming Range responses. Native requests require Tokio; WASM
  requests use Fetch and ReadableStream, not Tokio.
- Neither feature is enabled by default. `local` is unavailable on WASM even
  when its feature is enabled. Construct with `Local::new(root).await?` or
  `Web::new(url)?`; neither backend has a configurable read budget. Low-level
  `ReadAt` callers bypass SqPack's policy and must choose bounded lengths.
  Both backends still validate ranges, overflow and file consistency; Web
  always bounds the accumulated response by the requested length.

```rust,no_run
use icexiv_data::sqpack::{SqPack, web::Web};

// Compile with --features web; execute on a native Tokio runtime or a browser task.
async fn inspect_remote() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source = Web::new("https://assets.example/game/sqpack/")?;
    let sqpack = SqPack::new(source);
    Ok(sqpack.read("exd/item.exh").await?)
}
```

Resource lookup normalizes ASCII case, derives category/repository, and probes
the 256 chunk filenames until found. It prefers `.index` and falls back to
`.index2` only on confirmed container absence, never on a decode/IO failure.
A missing path may require 512 opens; indexes are not cached yet. Container
SHA-1 checksums are not verified. Use stable installations or version-pinned
HTTP roots: per-file validation does not provide a multi-file release snapshot.

The Web backend requires HEAD with `Content-Length` and a **strong ETag**.
Reads send `Range` and `If-Match`, require `206`, and validate the response ETag,
`Content-Range`, optional `Content-Length`, and exact body size. Missing/weak
ETags and servers ignoring Range are errors, not fallbacks to a full download.
Only a HEAD `404` means confirmed absence; read-time HTTP errors remain errors.
There is no HEAD fallback or support for query-token base URLs in this version.

Serve container bytes without HTTP content encoding. For cross-origin browser
requests, allow the origin and `If-Match` request header (plus `Range` when
required by the browser), and expose `ETag`, `Content-Range` and
`Content-Encoding`. Hidden response headers cannot be validated; CORS is a server
configuration requirement, not something the library can bypass. Native requests
have a 30-second timeout; browser callers should manage task cancellation.
Budgets bound accumulated bodies, not transport buffers or total concurrency.

The Local backend requires a trusted, stable installation (do not patch it while
reading). Length/mtime checks detect some mutations but are not snapshot
isolation. Canonicalization rejects resolved paths outside the root, but is not
a sandbox against concurrent filesystem changes. Unix uses `read_exact_at`;
Windows loops over `seek_read`, always supplying an explicit offset. Started
blocking tasks may continue after the awaiting future is cancelled.

### Design, dependency and test provenance

This access layer follows the API requirements discussed for this project.
Neither Physis implementation nor `legacy/` source/tests were consulted. The
format documentary inputs, license limitations and synthetic test provenance
are recorded in [SqPack provenance](docs/sqpack-provenance.md). This is not a
claim of certified clean-room independence. Backend implementation uses
the public APIs documented by [Tokio](https://docs.rs/tokio/1/tokio/task/fn.spawn_blocking.html)
(MIT), [reqwest](https://docs.rs/reqwest/0.12/reqwest/) (MIT OR Apache-2.0),
[futures-util](https://docs.rs/futures-util/0.3/futures_util/) (MIT OR Apache-2.0)
and Rust's standard-library positioned I/O. These are general-purpose I/O
dependencies, not game-format implementations; this records their direct
license declarations, not a completed transitive-license audit.

Tests use newly authored synthetic bytes, temporary files and in-memory HTTP
messages, without installed game data or a test server. They do not establish
SqPack format compatibility or browser end-to-end compatibility. This is not a
claim of strict clean-room independence for the project as a whole. Future
binary parsers require separately recorded approved specification/test provenance.

### Local resource example

```sh
cargo run --features local --example local_sqpack -- /path/to/game/sqpack exd/item.exh
```

Arguments are the SqPack root and a game resource path. The example extracts
standard binary entries through the built-in Local backend and prints their
length and first 64 bytes as hex, with a 64 MiB read/output cap. It does **not**
interpret the extracted EXH/EXD or other resource format. Specialized packed
model/texture entries remain unsupported.

The example uses the built-in Local backend on a current-thread Tokio runtime;
filesystem operations run off-thread. Other native platforms return
`Unsupported` for positioned reads.

## Development

```sh
nix develop
just check-all
```

Or use Cargo directly:

```sh
cargo fmt --all --check
cargo check --all-targets --all-features --locked
cargo test --all-features --locked
cargo check --no-default-features --locked
cargo check --lib --target wasm32-unknown-unknown --locked
cargo check --lib --target wasm32-unknown-unknown --all-features --locked
```

Install the WASM target first when not using Nix. `nix flake check` also verifies
the native/WASM builds and formatting in sandboxes. The local inspection
example above is optional; the library has no standalone CLI.

## Book

The project book lives in `docs/` and is built with Notist. Inside `nix develop`:

```sh
notist serve
notist build
```

The static book is written to `dist/icexiv-data-book/`.

## Legacy archive

When present locally, `legacy/` preserves the previous crate source (including
its CLI), Cargo manifest and lockfile. It is ignored by Git and is not included
in the new initial commit. Development tooling stays at the repository root.
The archive is not a workspace member, path
dependency or module of the active crate, and is excluded from the new Nix
build source. It retains its original dependencies and provenance; archiving
does not remove any applicable license obligations.

Treat this archive as historical material, not a specification or starter kit
for the new implementation. Do not read or reuse it during independent design
and implementation unless a separate, source-reviewed analysis task explicitly
requires it. Keeping old code in another directory is not by itself a strict
clean-room process.

Existing consumers can continue using their previously pinned Git revisions.
This new crate does not provide the old API or compatibility features.

Publication remains disabled pending an explicit project license decision.
