# autumn-plugin-svelte

Svelte 5 islands for [Autumn](https://github.com/autumn-foundation/autumn)
(`autumn-web` 0.8). The server renders an island in a Maud page. A small
loader mounts a compiled Svelte component into it. When htmx swaps HTML, the
loader mounts new islands and unmounts removed islands.

- You need no Node at run time. You deploy one binary.
- Autumn `PluginAssets` gives fingerprinted URLs, `immutable` cache and SRI
  hashes.
- The loader works with the default Autumn CSP (`script-src 'self'`).
- The page shows the server fallback when JavaScript is off or a mount fails.

## Install

After the first release on crates.io:

```toml
[dependencies]
autumn-plugin-svelte = "0.1"
```

Before the release, use the Git repository:

```toml
[dependencies]
autumn-plugin-svelte = { git = "https://github.com/autumn-foundation/autumn-plugin-svelte" }
```

## 1. Build your components

Use Vite and Svelte 5. The
[`frontend/`](https://github.com/autumn-foundation/autumn-plugin-svelte/tree/main/frontend)
directory is a complete reference project. These are the important parts:

```js
// vite.config.js
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: "../islands",
    emptyOutDir: true, // Vite keeps old files outside its root without this.
    lib: {
      entry: "src/main.js",
      formats: ["iife"], // a classic script, for <script defer>
      name: "Islands",
      fileName: () => "islands.js",
      cssFileName: "islands",
    },
  },
});
```

```js
// src/main.js: register the components with the loader.
import { mount, unmount } from "svelte";
import Counter from "./Counter.svelte";

let queue = window.autumnSvelte;
if (!Array.isArray(queue) && queue?.loader !== true) {
  queue = window.autumnSvelte = []; // an id="autumnSvelte" element can clobber it
}
queue.push({ mount, unmount, components: { Counter } });
```

Run `npm run build`. Commit the output.

An ES module build (`formats: ["es"]`) also works. Name the file
`islands.mjs`, and do not split it into chunks.

## 2. Embed the bundle and install the plugin

This code is in `src/main.rs`. The Vite output is in `islands/` at the crate
root, so the path is `../islands/`.

```rust
use autumn_plugin_svelte::SveltePlugin;
use autumn_web::assets::PluginAssets;

static ISLANDS: PluginAssets = PluginAssets::from_files(
    "app-islands", // any namespace except "svelte"
    &[
        ("islands.js", include_bytes!("../islands/islands.js")),
        ("islands.css", include_bytes!("../islands/islands.css")),
    ],
);

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        .plugin(SveltePlugin::new().bundle(&ISLANDS))
        .routes(autumn_web::routes![index])
        .run()
        .await;
}
```

With the `embed-assets` feature, `autumn_web::plugin_assets!` can embed the
whole directory.

A library crate can install its own `SveltePlugin` with its own bundles. The
plugin name includes the bundle namespaces, so the two plugins do not
collide.

## 3. Render islands

```rust
use autumn_plugin_svelte::{Island, MountWhen, svelte_bundle, svelte_script};
use autumn_web::prelude::*;

#[get("/")]
async fn index() -> AutumnResult<Markup> {
    let counter = Island::new("Counter")
        .props(&serde_json::json!({ "start": 3 }))?
        .fallback(html! { p { "Count: 3" } });
    Ok(html! {
        head {
            (svelte_script())          // the loader
            (svelte_bundle(&ISLANDS))  // your CSS and JS
        }
        body {
            (counter)
            (Island::new("Chart").mount_when(MountWhen::Visible))
        }
    })
}
```

| `Island` method | Effect |
|---|---|
| `new(name)` | Sets the registered component name. |
| `props(&value)` | Sets the JSON props. If the value is not a JSON object, `props` returns `PropsError`. `?` changes it to `AutumnError`. |
| `mount_when(MountWhen::…)` | Sets the trigger: `Load` (default), `Idle` or `Visible`. |
| `fallback(markup)` | Sets the server content. The page shows it before the mount, without JavaScript and after an error. |
| `id(…)`, `class(…)` | Set the attributes of the island element. |

## Lifecycle

The loader keeps one record for each island element.

| `data-svelte-state` | Meaning |
|---|---|
| `pending` | The loader waits for the trigger or for the component registration. |
| `mounted` | The component is mounted. |
| `error` | The props, the mount or the nesting depth failed. The loader shows the fallback. |

| Event | Target | `detail` |
|---|---|---|
| `autumn:svelte:mount` | island (bubbles) | `{ name }` |
| `autumn:svelte:error` | island (bubbles) | `{ name, error }` |
| `autumn:svelte:unmount` | `document` | `{ name, element }` |

- The loader mounts islands that htmx or a script adds later.
- The loader unmounts an island when it leaves the DOM. A moved island and
  an `hx-preserve` island stay mounted.
- When a morph (idiomorph) changes the island attributes or replaces its
  content, the loader unmounts the old component and mounts a new one.
- The loader keeps the fallback in a `<template data-svelte-fallback>`
  child. When htmx restores a page from history, the loader mounts its
  islands again.
- A bundle can register before or after the loader runs. A late
  registration mounts the islands that wait for it.
- The first registration of a name stays. A second one writes a console
  error.
- Errors in `onMount` or `$effect` occur after the mount. The island stays
  `mounted`.
- `window.autumnSvelte.scan(root)` mounts new islands in `root`. You
  usually do not need it.

## Security

- An island element mounts a real component with its props. Thus user HTML
  must not make islands. Put user HTML in an element with
  `data-svelte-ignore` (like htmx `hx-disable`). The loader does not mount
  islands in it:

  ```rust
  html! { div data-svelte-ignore { (PreEscaped(sanitized_comment)) } }
  ```

  Also tell your sanitizer to remove `data-svelte-*` attributes.
- A component that renders islands of itself stops at a depth of 16. The
  deepest island gets the `error` state.
- Props are JSON in an attribute. Maud escapes them, and the loader uses
  only `JSON.parse`.

## CSP

- The loader uses no `eval`, no inline script and no HTML parser. Thus it
  works with `script-src 'self'`.
- In Autumn nonce mode, `style-src` has no `'unsafe-inline'`. Then inline
  `style=` attributes in components do not work. Use component `<style>`
  blocks (they go into `islands.css`).
- With Trusted Types, allow the `svelte-trusted-html` policy of the Svelte
  runtime.
- `svelte_script()` has no `nonce`. A `'strict-dynamic'` policy blocks it.

## Limits

- The server does not render the component (Rust cannot run the Svelte
  compiler). The fallback is the first paint.
- You need Node to build your components.
- Props are in an HTML attribute. Keep them small.
- `svelte_bundle` makes tags for `.css`, `.js` and `.mjs` files only.

## Example

```sh
cargo run --example svelte_demo
```

Open <http://127.0.0.1:3000>. See
[`examples/svelte_demo.rs`](https://github.com/autumn-foundation/autumn-plugin-svelte/blob/main/examples/svelte_demo.rs).

## Tests

```sh
cargo test                                               # unit, doc and example tests
cargo test --test browser -- --ignored --test-threads=1  # Chromium tests
```

## Design

- [ADR 0001: Svelte islands through a plugin asset bundle](https://github.com/autumn-foundation/autumn-plugin-svelte/blob/main/docs/adr/0001-svelte-islands.md)
- [Plan and acceptance criteria](https://github.com/autumn-foundation/autumn-plugin-svelte/blob/main/docs/plan.md)

## License

Apache-2.0.
