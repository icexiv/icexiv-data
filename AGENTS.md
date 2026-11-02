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
