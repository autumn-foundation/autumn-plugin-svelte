# Plan: autumn-plugin-svelte

Target: `autumn-web` 0.8.0. Style: ASD-STE100.

No GitHub issue exists for this plugin. This plan writes the acceptance
criteria (AC) in [Acceptance criteria](#acceptance-criteria).

## Problem

Autumn renders HTML on the server (Maud + htmx). Some parts of a page need
rich client state. Svelte is a good tool for these parts. Autumn has no
standard way to add Svelte. Each app must write its own code for it.

## Brainstorming

1. Serve a small island loader (`islands.js`) as a `PluginAssets` bundle.
   The bundle gives fingerprinted URLs, `immutable` cache and SRI.
2. Give a typed Rust `Island` builder. It renders a Maud element with the
   component name and the JSON props.
3. Let the app bundle register its components on a queue
   (`window.autumnSvelte.push(...)`). Script order does not matter.
4. Each registration brings its own `mount` and `unmount` from `svelte`. The
   loader does not vendor a Svelte runtime, so the app selects the version.
5. Give mount strategies: `load`, `idle`, `visible`.
6. Mount new islands and unmount removed islands with one
   `MutationObserver`. This covers htmx swaps, morphs and manual DOM edits.
7. Keep server fallback content. Show it without JavaScript and after an
   error.
8. Send DOM events (`autumn:svelte:mount`, `autumn:svelte:unmount`,
   `autumn:svelte:error`), so htmx `hx-trigger` can react.
9. Install the app's compiled bundle through the plugin
   (`SveltePlugin::bundle`). The app gets the same URL and SRI rules.
10. Give a reference Vite + Svelte 5 project. Commit its output, so the
    example and the tests need no Node.
11. Vendor Svelte and use an import map. **Rejected**: an import map is an
    inline script, and CSP `script-src 'self'` blocks it. It also pins the
    Svelte version to the plugin.
12. Server-side render Svelte from Rust. **Rejected**: no Rust Svelte
    compiler exists. A Node process at run time stops the deploy of one
    binary.
13. An `autumn generate svelte` command. **Deferred**: it needs a change in
    `autumn-cli`.

## Reverse brainstorming

Question: "How can this plugin fail?" Each answer gives a control.

| Failure | Control |
|---|---|
| An htmx swap removes an island, and the component leaks. | The observer unmounts each removed island. |
| A re-scan mounts one island two times. | The loader keeps one record per element (`WeakMap`). |
| Props inject HTML or script. | Props go only in an attribute. Maud escapes it. The loader uses `JSON.parse`, never `innerHTML` or `eval`. |
| The name `__proto__` or `constructor` reads `Object.prototype`. | The registry is a `Map`, and it copies only own properties. |
| The plugin breaks the default CSP. | No inline script, no `eval`, no inline import map. |
| A browser keeps old bytes after an upgrade. | Fingerprinted URLs and SRI from `PluginAssets`. |
| The app bundle runs before the loader. | The queue pattern. The loader drains the queue at start. |
| A component throws in `mount`. | The loader puts the fallback back, sets `data-svelte-state="error"` and sends `autumn:svelte:error`. |
| Two bundles use different Svelte versions. | Each component mounts with the `mount` of its own bundle. |
| Two bundles register the same name. | The first registration stays. The loader writes a console error. |
| A morph moves an island (remove, then add). | The loader unmounts only when the element is not connected. |
| The island goes away before `idle` or `visible` fires. | Teardown cancels the pending trigger. |
| htmx history restore brings back stale Svelte DOM and no fallback. | The loader keeps the fallback in a `<template data-svelte-fallback>` child. A restored island mounts again from clean state. |
| Props are not a JSON object (`5`, `[1]`). | `Island::props` returns `PropsError::NotAnObject`. |
| The app bundle uses the namespace `svelte`. | `SveltePlugin::bundle` stops with a clear message. |
| A second `SveltePlugin` (for example, in a library crate) has other bundles. | The plugin name includes the bundle namespaces, so Autumn installs both plugins. |
| User HTML contains an island and mounts a real component. | `data-svelte-ignore` blocks islands in user HTML. The docs tell the app to remove `data-svelte-*` in the sanitizer. |
| A component renders islands of itself without end and freezes the tab. | The loader stops at a nesting depth of 16. |
| An element with `id="autumnSvelte"` clobbers the global. | The registration snippet checks the global before it uses it. |
| An idiomorph morph changes an island in place. | The loader watches island attributes and the fallback template. It mounts the island again. |
| No JavaScript. | The fallback content stays. |
| The loader script loads two times. | The second copy does nothing. |

## Six thinking hats

- **White (facts).** `autumn-web` 0.8.0 has `PluginAssets::from_files`,
  `deferred_script_tag`, `stylesheet_tag` and `AppBuilder::plugin_assets`.
  The default CSP is `script-src 'self'; style-src 'self' 'unsafe-inline'`.
  Svelte 5 has `mount(Component, { target, props })` and
  `unmount(instance)`. `hydrate` needs Svelte SSR markup, which Rust cannot
  make. A second plugin with the same name is a no-op.
- **Red (feelings).** Developers want to write a `.svelte` file and put it
  in a Maud page. Node to build components is acceptable. Node at run time
  is not. Developers do not like to configure a bundler, so the plugin gives
  a reference project.
- **Black (risks).** JavaScript is not tested by `cargo test` alone. Control:
  real Chromium tests through `autumn_web::system_test`. Risk: Svelte API
  change in a later major version. Control: the app owns `mount`/`unmount`.
  Risk: large props in an attribute. Control: document it.
- **Yellow (benefits).** Rich islands in an htmx app. Cache and SRI with no
  work. You deploy one binary. Each island is independent.
- **Green (new ideas).** Mount strategies. DOM events for htmx.
  `svelte_bundle` emits all CSS and JS tags of a bundle.
- **Blue (process).** We use RED, GREEN, REFACTOR for each unit. Then we
  review the code from several aspects. Then we make an AC evidence table.

## Design summary

See [ADR 0001](adr/0001-svelte-islands.md).

## Acceptance criteria

1. **AC1.** `SveltePlugin::new()` installs the loader bundle under
   `/static/_plugins/svelte/` with fingerprinted, `immutable` URLs and SRI.
2. **AC2.** `SveltePlugin::bundle(&BUNDLE)` installs an app bundle of
   compiled Svelte components with the same URL and cache rules.
3. **AC3.** `svelte_script()` and `svelte_bundle(&BUNDLE)` emit `<script>`
   and `<link>` tags with SRI and `crossorigin="anonymous"`.
4. **AC4.** `Island` renders a Maud element with the component name, JSON
   props, mount strategy, fallback, `id` and `class`. Bad props give a typed
   error. Props are escaped.
5. **AC5.** The loader mounts a real Svelte 5 component in Chromium with its
   props. The default Autumn CSP applies. The console has no errors.
6. **AC6.** The loader mounts islands that htmx swaps in, and unmounts
   islands that htmx swaps out.
7. **AC7.** `idle` and `visible` strategies wait for their trigger.
8. **AC8.** A failed mount keeps the fallback, sets
   `data-svelte-state="error"` and sends `autumn:svelte:error`.
9. **AC9.** Script order does not matter (queue), and one element mounts one
   time only.
10. **AC10.** The plugin passes `autumn_web::plugin_conformance`, declares a
    `PluginContract` for `autumn-web` 0.8, and a second install is harmless.
11. **AC11.** A runnable example and a reference Vite + Svelte 5 project
    exist. The example runs with no Node.
12. **AC12.** README, ADR, CHANGELOG and doc comments are short and use
    ASD-STE100. CI runs fmt, clippy (pedantic, nursery), tests, browser
    tests and a bundle freshness check.
