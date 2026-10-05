// Registers the components with the Autumn island loader.
// The loader can run before or after this file.
import { mount, unmount } from "svelte";
import Counter from "./Counter.svelte";
import Clock from "./Clock.svelte";

// An element with id="autumnSvelte" can replace the global, so check it.
let queue = window.autumnSvelte;
if (!Array.isArray(queue) && queue?.loader !== true) {
  queue = window.autumnSvelte = [];
}
queue.push({ mount, unmount, components: { Counter, Clock } });
