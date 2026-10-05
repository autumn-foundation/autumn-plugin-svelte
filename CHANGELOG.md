# Changelog

## [Unreleased]

### Added

- `SveltePlugin`: installs the island loader and app component bundles as
  Autumn `PluginAssets` (fingerprinted URLs, `immutable` cache, SRI).
- `Island`: renders a Svelte island with JSON props, a mount strategy
  (`MountWhen::Load`, `Idle`, `Visible`), fallback content, `id` and
  `class`.
- `svelte_script()` and `svelte_bundle()`: `<script>` and `<link>` tags with
  SRI.
- `islands.js` loader: mounts and unmounts islands on htmx swaps, keeps the
  fallback, reports errors with DOM events.
- Reference Vite + Svelte 5 project (`frontend/`) and the `svelte_demo`
  example.
