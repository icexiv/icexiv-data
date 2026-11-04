# Repository guidance

- The active project is a fresh, single Rust library crate in `src/`, currently
  with no dependencies, features or CLI. Do not prepopulate its design with the
  old module tree or compatibility APIs.
- `legacy/` is a historical archive, not part of the active build. Do not read,
  copy, import, or depend on its source or tests during new design and
  implementation unless the user explicitly requests a separate source audit.
  Do not use it as a format specification or an implementation template.
- Do not add Physis or other copyleft implementation dependencies without an
  explicit licensing decision. Review sources and licenses before consulting
  external implementations; record approved specification and test provenance.
  Do not claim strict clean-room independence merely from moving or rewriting
  code already seen by the implementer.
- Keep binary parsing independent of filesystem, browser, HTTP, async runtime,
  and GPU integration. Validate lengths, offsets, arithmetic and resource
  budgets; distinguish confirmed absence from IO, permission and decode errors.
- Preserve the archived source and any applicable license/provenance notices.
  Publication is disabled pending an explicit project license decision.
- Do not start servers or run installed-game/GPU tests without an explicit
  request. Use `just check-all` or `nix flake check` for ordinary verification.
- Keep the README about the current library. Do not create a changelog.

## Documentation content

- Write documentation as a reference, not a development log, progress report or
  explanation of the changes you made. Describe the subject directly.
- Format documentation under `docs/` starts from the FFXIV format itself: its
  purpose, file structure, fields, offsets, types, encodings, variants and their
  relationships. Its scope and organization must not be dictated by which parts
  this library implements. For example, a TEX format page describes texture
  types and pixel formats, not a list of formats supported by the decoder.
- Do not use temporal or change-oriented framing such as “当前”, “目前”, “暂时”,
  “尚未”, “不再”, “新增”, “后续”, “current”, “not yet” or “now supports”. Changing
  the heading from “当前支持” to “支持的格式” does not fix an implementation-centered
  format document; rewrite its content and point of view.
- Explain what a thing is and how it is structured. Avoid unsolicited
  “不是……而是……” contrasts, redundant boundary disclaimers and reminders of
  already established architecture. Include distinctions only when they resolve
  a genuine ambiguity needed to understand the format.
- Keep format reference, library usage and provenance separate. API usage,
  supported implementation subsets, error behavior and resource budgets belong
  in Rust API documentation or the library README. Source/license review and
  test provenance belong in a separate provenance document; preserve those
  records and notices rather than deleting them from the project.
- Identify the platform, version or format variant where relevant. Distinguish
  documented facts from inference and unknown fields; do not invent meanings or
  present an implementation restriction as a file-format rule.

## Pele diagrams

- Use the repository's Pele component where a diagram makes the format easier
  to understand. In particular, illustrate binary file headers and record
  layouts with Pele, following `docs/sqpack/container-formats.md` and
  `docs/tex/README.md`; do not default to tables alone.
- Choose diagrams flexibly for the subject: packet layouts for byte/bit fields,
  bitfield diagrams for flags, and suitable diagrams for structural relationships
  or data organization. Do not force every paragraph or enumeration into a
  diagram. Tables are useful for field meanings and format IDs, and may accompany
  a layout diagram without replacing it.
- Import Pele using the path relative to the document, for example
  `#import "../_components/pele/lib.notc" as pele;`, and use
  `#pele.diagram(r#""" … """#)` following the existing document syntax.
- State the coordinate unit and origin. Packet diagram positions are in bits;
  explain whether offsets are relative to the file, a header or a record. State
  byte order separately. For numeric bitfields, make low/high bit numbering and
  its distinction from byte order explicit.
- Keep diagrams consistent with field tables and size calculations. Validate
  documentation changes with `nix develop -c notist build`; do not start a server
  merely to check documentation.
