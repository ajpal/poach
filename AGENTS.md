## Rust instructions

- Make minimal, localized changes.
- Match existing style and crate structure.
- Use idiomatic Rust and `cargo fmt`.
- Prefer readability over cleverness.
- Propagate errors with `Result` and `?`.
- Prefer borrowing over cloning.
- Add or update tests for behavior changes.
- Run:
  - `cargo fmt --all`
  - `cargo check`
- Do not use async code without explicit permission.
- Do not use unsafe code without explicit permission.
- Do not add or change dependencies without explicit permission.
- Do not use lifetime annotations without explicit permission.

## Conventions + Style

- Avoid defensive code that adds complexity without a clear, likely payoff.
- Prefer simple `expect("...")` messages over custom `unwrap_or_else` panic paths when the extra detail is not useful.
- Do not add incidental determinism, cleanup, or abstraction unless it is required for correctness or a clear user need.
- Do not introduce a helper that is only used once unless it meaningfully improves readability.
- Do not add convenience wrappers that only forward to an existing API; prefer calling the existing API directly at the call site.

## Response format

- Small, reviewable code diff (about the size of one commit) with a proposed commit message.
- Explain any assumptions or decisions you made.
