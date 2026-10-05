// Registers the components with the Autumn island loader.
// The loader can run before or after this file.
import { mount, unmount } from "svelte";
import Counter from "./Counter.svelte";
import Clock from "./Clock.svelte";

(window.autumnSvelte = window.autumnSvelte || []).push({
  mount,
  unmount,
  components: { Counter, Clock },
});
