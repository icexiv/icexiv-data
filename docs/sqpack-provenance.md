# SqPack implementation provenance and scope

## Scope

The implemented resource path is `SqPack::read(path)` to bytes, for Win32
little-endian version-1 containers and **type-2 standard binary** entries. It
supports primary indexes, index2-only packs, raw blocks and raw DEFLATE blocks.
It does not interpret EXH/EXD, texture, model or other extracted resource formats.

Unsupported packed content types (including placeholders, models and textures)
return `UnsupportedContentType`. Flagged/synonym records and ambiguous duplicate
hashes return `UnsupportedIndexEntry`, never guessed data or
confirmed absence. No SHA-1 verification or index caching is implemented.

No directory listing or user-provided file catalog is required. Routing selects
the category and repository from the normalized resource path, then probes the
two-hex-digit chunk namespace in ascending order (`00` through `ff`). This is a
deliberately exhaustive routing policy, not a claim to infer a chunk from the
resource path. It prefers `.index`; `.index2` is used only when `.index` is
confirmed absent. A miss can perform 512 container opens. Performance and
compatibility beyond synthetic fixtures are not yet established.

## Documentary inputs consulted on 2026-11-02

Only documentary files listed here were selected as game-format inputs; no
implementation source files from these repositories were fetched. The documents
include implementation-derived layout declarations, and one explicitly describes
knowledge gathered from other programs. This is therefore a documented,
documentation-guided implementation, **not a certification of strict clean-room
independence**. No Physis implementation or `legacy/` source/tests were consulted.

1. [quilan1/ffxiv-parser-lib, sqpack.md](https://github.com/quilan1/ffxiv-parser-lib/blob/759ead4bbe0d6897efe9b76eebe0a7c69c72c873/sqpack.md)
   - Last file revision: `759ead4bbe0d6897efe9b76eebe0a7c69c72c873`.
   - Repository license reviewed before opening the document: MIT,
     Copyright (c) 2024 quilan1.
   - Used the factual description of JAMCRC, directory/file hashes, index and
     index2 records, bit-packed data location, standard entry/block sizes and
     raw DEFLATE. The documented directory hash excludes the final slash; this
     resolves an ambiguity in the wiki's prose using its explicit hash example.
   - Did not copy its game byte dumps or EXH/EXD layouts into tests.

2. [XIV Dev Wiki, SqPack](https://github.com/xivdev/docs/blob/6b33f21e833e22351580fcd193da8e99fce5d865/content/data-files/sqpack.md)
   - Last file revision: `6b33f21e833e22351580fcd193da8e99fce5d865`.
   - No repository license was discoverable through the license endpoint or
     root listing. This is **not** recorded as a licensed code source: only
     factual category/repository/platform descriptions were used; its prose
     and C++/C# declarations were not copied into this crate.
   - Consulted its category IDs, repository selection and index2 description.
     Its linked implementation sources were not followed.

3. [yeliex/sqpack, README.md, File Segments](https://github.com/yeliex/sqpack/blob/aaf1f0cad898d8720fe7bfc7d406ac7b54ec792f/README.md)
   - Last file revision: `aaf1f0cad898d8720fe7bfc7d406ac7b54ec792f`.
   - Mirror repository declares MIT, Copyright (c) 2018 yeliex. License was
     reviewed before opening the document.
   - The research text attributes itself to **Ioncannon**, including a DAT
     research date of 2014-12-14. The mirror's license does not independently
     establish the original research text's licensing history.
   - Used factual standard-block fields, the `32000` raw-block marker and
     minimum entry header size. No research tables, prose or unrelated code
     snippets from the README were copied into the implementation or fixtures.
     Its incomplete model/texture descriptions were not used to guess decoders.

These recorded license limitations must not be represented as a completed
publication/provenance clearance. Project publication remains disabled. A
separate licensing decision is still required before distributing the crate.

## General-purpose dependencies

`binrw` 0.15 (MIT) provides fixed-size, explicitly little-endian `BinRead`
layouts. Its public API and license were reviewed before adoption. It reads
only from `Cursor<&[u8]>` over bytes already acquired through the backend, never
directly from files or HTTP. Magic/padding/field reading use the derive; span
checks, count validation, budgets, collision rules and decompression validation
remain explicit. No file-provided count is passed to a derive-managed `Vec`.

`crc32fast` 1 (MIT OR Apache-2.0) is a direct dependency for the existing JAMCRC
operation: complement its standard IEEE CRC-32 result. The previously documented
hash vectors are retained, including the Python cross-checks. This replaces a
handwritten CRC loop without consulting another game-format implementation.

`flate2` is selected with `default-features = false` and `rust_backend`, using
pure-Rust DEFLATE rather than a native zlib dependency. Its declared license is
MIT OR Apache-2.0. The documented `Decompress` API is used to require stream end,
exact input consumption and exact output size. Temporary decoded block storage
is bounded by the block table's u16 length plus one overflow-detection byte.

Public API references: [binrw](https://docs.rs/binrw/0.15/binrw/),
[crc32fast::hash](https://docs.rs/crc32fast/1/crc32fast/fn.hash.html),
[flate2::Decompress](https://docs.rs/flate2/1/flate2/struct.Decompress.html).
This is not a completed transitive-license audit.

## Test provenance

- All fixture container files are newly assembled in `tests/sqpack_resource.rs`.
  They contain arbitrary author-created bytes, not game extracts or another
  implementation's fixtures. Some are compressed with flate2 for round-trip
  coverage, which alone is not an independent decompressor oracle.
- Hash vectors for `exd` and `item.exh` are identified by the first document.
  They were cross-checked with Python `zlib.crc32(...) ^ 0xffffffff`.
- The full-path vector and fixed `hello SqPack` compressed stream were
  independently generated using Python zlib (`compressobj(wbits=-15)`), without
  any SqPack library. Format-unit tests include this fixed raw-DEFLATE stream,
  truncation, trailing data and excess-output checks.
- Layout tests assert the exact fixed-prefix size and reject every truncated
  prefix for all seven derived headers/records. Existing synthetic end-to-end
  fixtures continue to write fields independently, without using BinWrite.
- Tests use an in-memory source and the existing temporary-file/native backend
  tests. No installed game, game executable, HTTP server or GPU was used. Browser
  compatibility is compile-checked, not end-to-end verified.

## Documentary reference license notices

The following repository notices are retained conservatively for the two
MIT-declared documentary references. They do not resolve the separate wiki or
original Ioncannon research licensing questions noted above.

Copyright (c) 2024 quilan1

Copyright (c) 2018 yeliex

MIT License

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
