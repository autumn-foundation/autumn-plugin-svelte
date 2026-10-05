# CLAUDE.md: autumn-plugin-svelte

Svelte 5 islands for `autumn-web` 0.8.

## Layout

| Path | Content |
|---|---|
| `src/island.rs` | `Island`, `MountWhen`, `PropsError`. |
| `src/plugin.rs` | `SveltePlugin`. |
| `src/tags.rs` | `svelte_script`, `svelte_bundle`. |
| `src/assets.rs` | `SVELTE_ASSETS` (the loader bundle). |
| `assets/islands.js` | The browser loader. No build step. |
| `frontend/` | Reference Vite + Svelte 5 project. Builds `examples/islands/`. |
| `tests/browser.rs` | Chromium tests (`#[ignore]`). |
| `tests/fixtures/fake.js` | Fake components for the loader contract. |
| `tests/fixtures/probe.js` | Holds idle callbacks and counts observer disconnects. |
| `tests/fixtures/module.mjs` | An ES-module bundle. |

## Commands

- Lint: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
- Test: `cargo test`
- Browser tests: `cargo test --test browser -- --ignored --test-threads=1`
  (screenshots of failures go to `target/system-tests/`)
- Docs: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`
- Rebuild the bundle: `cd frontend && npm ci && npm run build`

## Rules

- Write docs and comments in ASD-STE100: short sentences, active voice.
- After a change in `frontend/src/`, rebuild and commit `examples/islands/`.
  CI fails on a stale bundle.
- The loader must not use `eval`, `new Function`, `innerHTML` or
  `document.write`. A unit test checks this.
- A loader change needs a browser test. A Rust change needs a unit test.
- Make sure that a new test fails before the fix (RED), then passes (GREEN).
- Do not add a dependency on a Svelte runtime to the loader. Each app bundle
  brings its own `mount` and `unmount`.
