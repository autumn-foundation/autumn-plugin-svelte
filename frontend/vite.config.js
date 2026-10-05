// Builds the island bundle: one classic script and one stylesheet.
// Output: ../examples/islands/islands.js and islands.css.
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: {
    outDir: "../examples/islands",
    emptyOutDir: true,
    // Keep the output byte-stable, so CI can check that it is fresh.
    reportCompressedSize: false,
    lib: {
      entry: "src/main.js",
      // A classic script: `svelte_bundle` emits `<script defer>`.
      formats: ["iife"],
      name: "AutumnSvelteIslands",
      fileName: () => "islands.js",
      cssFileName: "islands",
    },
  },
});
