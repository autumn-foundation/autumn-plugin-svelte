# Changelog

## [Unreleased]

### Added

- `SveltePlugin`: installs the island loader and app component bundles as
  Autumn `PluginAssets` (fingerprinted URLs, `immutable` cache, SRI). The
  plugin name includes the bundle namespaces, so a library crate and the app
  can each install one.
- `Island`: renders a Svelte island with JSON props, a mount strategy
  (`MountWhen::Load`, `Idle`, `Visible`), fallback content, `id` and
  `class`.
- `svelte_script()` and `svelte_bundle()`: `<script>` and `<link>` tags with
  SRI. `.mjs` files get `<script type="module">`.
- `PropsError` and `JsonKind`: the error when props are not a JSON object.
- `islands.js` loader: mounts and unmounts islands on htmx swaps and morphs,
  keeps the fallback, and sends DOM events for mount, unmount and error.
  Islands in a `data-svelte-ignore` element never mount. Islands stop at a
  nesting depth of 16.
- Reference Vite + Svelte 5 project (`frontend/`) and the `svelte_demo`
  example.
