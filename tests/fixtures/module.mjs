/* Test bundle as an ES module. svelte_bundle emits <script type="module">. */
const mount = (Component, options) => Component(options.target, options.props);
const unmount = () => {};

function Module(target, props) {
  target.append("module " + props.k);
  return {};
}

let queue = window.autumnSvelte;
if (!Array.isArray(queue) && queue?.loader !== true) {
  queue = window.autumnSvelte = [];
}
queue.push({ mount, unmount, components: { Module } });

export {};
