# autumn-plugin-svelte

Svelte 5 islands for [Autumn](https://github.com/autumn-foundation/autumn)
(`autumn-web` 0.8). The server renders an island in a Maud page. A small
loader mounts a compiled Svelte component into it. htmx swaps mount and
unmount islands automatically.

- No Node at run time. One binary at deploy.
- Fingerprinted, `immutable` URLs and SRI hashes from Autumn `PluginAssets`.
- Works with the default Autumn CSP (`script-src 'self'`).
- Server fallback content without JavaScript, and after a failed mount.

## Install

```toml
[dependencies]
autumn-plugin-svelte = "0.1"
```

## 1. Build your components

Use Vite and Svelte 5. The [`frontend/`](frontend/) directory is a complete
reference project. The important parts:

```js
// vite.config.js
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: "../islands",
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

(window.autumnSvelte = window.autumnSvelte || []).push({
  mount,
  unmount,
  components: { Counter },
});
```

Run `npm run build`. Commit the output.

## 2. Embed the bundle and install the plugin

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

autumn_web::app()
    .plugin(SveltePlugin::new().components(&ISLANDS))
    // ...
```

With the `embed-assets` feature, `autumn_web::plugin_assets!` can embed the
whole directory.

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
| `new(name)` | The registered component name. |
| `props(&value)` | JSON props. The value must be a JSON object, else `PropsError`. `?` converts it to `AutumnError`. |
| `mount_when(MountWhen::…)` | `Load` (default), `Idle`, or `Visible`. |
| `fallback(markup)` | Server content. Shown before the mount, without JavaScript, and after an error. |
| `id(…)`, `class(…)` | Attributes of the island element. |

## Lifecycle

The loader keeps one record for each island element.

| `data-svelte-state` | Meaning |
|---|---|
| `pending` | The loader waits for the trigger or for the component registration. |
| `mounted` | The component is mounted. |
| `error` | The props or the mount failed. The fallback shows. |

| Event | Target | `detail` |
|---|---|---|
| `autumn:svelte:mount` | island (bubbles) | `{ name }` |
| `autumn:svelte:error` | island (bubbles) | `{ name, error }` |
| `autumn:svelte:unmount` | `document` | `{ name, element }` |

- The loader mounts islands that the DOM gets later (htmx swaps, morphs).
- The loader unmounts an island when it leaves the DOM. A moved island
  stays mounted.
- The loader keeps the fallback in a `<template data-svelte-fallback>`
  child. Markup that comes back from htmx history mounts again, clean.
- A bundle can register before or after the loader runs. A late
  registration mounts the islands that wait for it.
- `window.autumnSvelte.scan(root)` mounts new islands in `root`. You do not
  usually need it.

## Limits

- No server-side render of the component (Rust cannot run the Svelte
  compiler). The fallback is the first paint.
- You need Node to build your components.
- Props are in an HTML attribute. Keep them small.
- `svelte_bundle` emits tags for `.css` and `.js` files only. Build one
  classic script (`formats: ["iife"]`).

## Example

```sh
cargo run --example svelte_demo
```

Open <http://127.0.0.1:3000>. See [`examples/svelte_demo.rs`](examples/svelte_demo.rs).

## Tests

```sh
cargo test                                                      # unit and doc tests
cargo test --test browser -- --ignored --test-threads=1         # Chromium tests
```

## Design

- [ADR 0001: Svelte islands through a plugin asset bundle](docs/adr/0001-svelte-islands.md)
- [Plan and acceptance criteria](docs/plan.md)

## License

Apache-2.0.
