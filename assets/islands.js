/* autumn-plugin-svelte: island loader.
 *
 * Finds [data-svelte-island] elements and mounts Svelte components into them.
 * App bundles register components on a queue, so script order does not
 * matter:
 *
 *   let queue = window.autumnSvelte;
 *   if (!Array.isArray(queue) && queue?.loader !== true) {
 *     queue = window.autumnSvelte = [];
 *   }
 *   queue.push({ mount, unmount, components: { Counter } });
 *
 * One MutationObserver mounts added islands and unmounts removed islands
 * (htmx swaps, morphs, manual DOM changes).
 *
 * Element states (data-svelte-state): pending, mounted, error.
 * Events: autumn:svelte:mount and autumn:svelte:error on the island (they
 * bubble); autumn:svelte:unmount on document (the island is detached).
 *
 * No eval, no inline script, no HTML parsing: it works with
 * CSP script-src 'self'.
 */
(function () {
  "use strict";

  var existing = window.autumnSvelte;
  if (existing && existing.loader === true) {
    return; // This file is on the page two times.
  }

  var SELECTOR = "[data-svelte-island]";
  // Islands in an element with this attribute never mount. Wrap user HTML
  // in it, as with htmx hx-disable.
  var IGNORE = "[data-svelte-ignore]";
  // A component that renders islands of itself stops at this depth.
  var MAX_DEPTH = 16;
  var STATE = "data-svelte-state";
  var FALLBACK = "data-svelte-fallback";

  // name -> { component, mount, unmount }. A Map, so names such as
  // "__proto__" and "constructor" are plain keys.
  var registry = new Map();
  // element -> record. A record is deleted at teardown.
  var records = new WeakMap();
  // Elements that wait for a component or a trigger.
  var pending = new Set();

  function emit(target, type, detail) {
    target.dispatchEvent(new CustomEvent(type, { bubbles: true, detail: detail }));
  }

  function isElement(node) {
    return node !== null && typeof node === "object" && node.nodeType === 1;
  }

  // The direct <template data-svelte-fallback> child, or null.
  function fallbackOf(el) {
    for (var child = el.firstElementChild; child; child = child.nextElementSibling) {
      if (child.tagName === "TEMPLATE" && child.hasAttribute(FALLBACK)) {
        return child;
      }
    }
    return null;
  }

  // Removes every child except the fallback template.
  function clearExceptFallback(el, template) {
    var child = el.firstChild;
    while (child) {
      var next = child.nextSibling;
      if (child !== template) {
        el.removeChild(child);
      }
      child = next;
    }
  }

  // Moves the fallback content into a template child and returns it.
  function stashFallback(el) {
    var template = fallbackOf(el);
    if (template) {
      return template;
    }
    template = document.createElement("template");
    template.setAttribute(FALLBACK, "");
    while (el.firstChild) {
      template.content.appendChild(el.firstChild);
    }
    el.appendChild(template);
    return template;
  }

  // Puts the fallback content back and removes the template.
  function restoreFallback(el, template) {
    clearExceptFallback(el, template);
    el.appendChild(template.content);
    el.removeChild(template);
  }

  function parseProps(el) {
    var raw = el.getAttribute("data-svelte-props");
    if (raw === null) {
      return {};
    }
    var props = JSON.parse(raw);
    if (props === null || typeof props !== "object" || Array.isArray(props)) {
      throw new TypeError("data-svelte-props must be a JSON object");
    }
    return props;
  }

  function fail(el, record, error) {
    record.state = "error";
    pending.delete(el);
    el.setAttribute(STATE, "error");
    console.error("autumn-svelte: island failed to mount:", record.name, error);
    emit(el, "autumn:svelte:error", { name: record.name, error: error });
  }

  // The number of island ancestors of `el`.
  function depthOf(el) {
    var depth = 0;
    var ancestor = el.parentElement && el.parentElement.closest(SELECTOR);
    while (ancestor && depth < MAX_DEPTH) {
      depth++;
      ancestor = ancestor.parentElement && ancestor.parentElement.closest(SELECTOR);
    }
    return depth;
  }

  function tryMount(el) {
    var record = records.get(el);
    if (!record || record.state !== "pending" || !record.ready || !el.isConnected) {
      return;
    }
    var entry = registry.get(record.name);
    if (!entry) {
      return; // Stays pending until a bundle registers the name.
    }
    if (depthOf(el) >= MAX_DEPTH) {
      fail(el, record, new Error("islands are nested deeper than " + MAX_DEPTH));
      return;
    }
    var props;
    try {
      props = parseProps(el);
    } catch (error) {
      fail(el, record, error);
      return;
    }
    var template = stashFallback(el);
    // "mounting" stops a second mount when mount() registers a bundle.
    record.state = "mounting";
    pending.delete(el);
    var instance;
    try {
      instance = entry.mount(entry.component, { target: el, props: props });
    } catch (error) {
      if (records.get(el) === record) {
        restoreFallback(el, template);
        fail(el, record, error);
      }
      return;
    }
    record.instance = instance;
    record.entry = entry;
    record.template = template;
    record.state = "mounted";
    if (records.get(el) !== record) {
      // mount() removed its own island. Unmount the new instance.
      records.set(el, record);
      teardown(el);
      return;
    }
    el.setAttribute(STATE, "mounted");
    emit(el, "autumn:svelte:mount", { name: record.name });
  }

  function ready(el) {
    var record = records.get(el);
    if (record) {
      record.ready = true;
      record.cancel = null;
      tryMount(el);
    }
  }

  // Calls ready(el) when the mount trigger fires. Returns a cancel function.
  function schedule(el, when) {
    if (when === "visible" && typeof IntersectionObserver === "function") {
      var observer = new IntersectionObserver(function (entries) {
        for (var i = 0; i < entries.length; i++) {
          if (entries[i].isIntersecting) {
            observer.disconnect();
            ready(el);
            return;
          }
        }
      });
      observer.observe(el);
      return function () {
        observer.disconnect();
      };
    }
    if (when === "idle") {
      if (typeof requestIdleCallback === "function") {
        var idle = requestIdleCallback(function () {
          ready(el);
        }, { timeout: 2000 });
        return function () {
          cancelIdleCallback(idle);
        };
      }
      var timer = setTimeout(function () {
        ready(el);
      }, 1);
      return function () {
        clearTimeout(timer);
      };
    }
    ready(el);
    return null;
  }

  function setup(el) {
    if (!el.isConnected || el.closest(IGNORE)) {
      return;
    }
    if (records.has(el)) {
      tryMount(el); // A pending island can be ready now.
      return;
    }
    // Markup from an earlier mount (htmx history restore, a clone) has a
    // fallback template and stale component DOM. Put the fallback back.
    var stale = fallbackOf(el);
    if (stale) {
      restoreFallback(el, stale);
    }
    var record = {
      name: el.getAttribute("data-svelte-island") || "",
      props: el.getAttribute("data-svelte-props"),
      when: el.getAttribute("data-svelte-mount"),
      state: "pending",
      ready: false,
      cancel: null,
      instance: null,
      entry: null,
      template: null,
    };
    records.set(el, record);
    pending.add(el);
    el.setAttribute(STATE, "pending");
    var cancel = schedule(el, record.when);
    if (records.get(el) === record && !record.ready) {
      record.cancel = cancel;
    }
  }

  function teardown(el) {
    var record = records.get(el);
    if (!record) {
      return;
    }
    records.delete(el);
    pending.delete(el);
    el.removeAttribute(STATE);
    if (record.cancel) {
      record.cancel();
    }
    if (record.state !== "mounted") {
      return;
    }
    var logUnmountError = function (error) {
      console.error("autumn-svelte: island failed to unmount:", record.name, error);
    };
    try {
      // Svelte 5 unmount() can return a Promise.
      Promise.resolve(record.entry.unmount(record.instance)).catch(logUnmountError);
    } catch (error) {
      logUnmountError(error);
    }
    emit(document, "autumn:svelte:unmount", { name: record.name, element: el });
  }

  // Islands in `root`, `root` included.
  function islandsIn(root) {
    var found = [];
    if (isElement(root) && root.matches(SELECTOR)) {
      found.push(root);
    }
    if (root && typeof root.querySelectorAll === "function") {
      var list = root.querySelectorAll(SELECTOR);
      for (var i = 0; i < list.length; i++) {
        found.push(list[i]);
      }
    }
    return found;
  }

  function scan(root) {
    islandsIn(root || document).forEach(setup);
  }

  function register(entry) {
    if (
      !entry ||
      typeof entry.mount !== "function" ||
      typeof entry.unmount !== "function" ||
      !entry.components ||
      typeof entry.components !== "object"
    ) {
      console.error("autumn-svelte: push() needs { mount, unmount, components }", entry);
      return;
    }
    Object.keys(entry.components).forEach(function (name) {
      if (registry.has(name)) {
        console.error("autumn-svelte: component already registered; the first one stays:", name);
        return;
      }
      registry.set(name, {
        component: entry.components[name],
        mount: entry.mount,
        unmount: entry.unmount,
      });
    });
    Array.from(pending).forEach(tryMount);
  }

  // True when something other than the loader changed a recorded island:
  // its attributes, its state attribute, or its fallback template (an
  // in-place morph does this).
  function changedInPlace(el, record) {
    return (
      el.getAttribute("data-svelte-island") !== record.name ||
      el.getAttribute("data-svelte-props") !== record.props ||
      el.getAttribute("data-svelte-mount") !== record.when ||
      el.getAttribute(STATE) !== visibleState(record) ||
      (record.template !== null && record.template.parentNode !== el)
    );
  }

  // The state attribute value the loader writes for a record.
  function visibleState(record) {
    return record.state === "mounting" ? "pending" : record.state;
  }

  function remount(el) {
    teardown(el);
    if (el.hasAttribute("data-svelte-island")) {
      setup(el);
    }
  }

  function onMutations(mutations) {
    var touched = new Set();
    for (var i = 0; i < mutations.length; i++) {
      var target = mutations[i].target;
      if (mutations[i].type === "attributes") {
        touched.add(target);
        continue;
      }
      if (records.has(target)) {
        touched.add(target); // Its children changed.
      }
      var removed = mutations[i].removedNodes;
      for (var r = 0; r < removed.length; r++) {
        islandsIn(removed[r]).forEach(function (el) {
          if (!el.isConnected) {
            teardown(el);
          }
        });
      }
      var added = mutations[i].addedNodes;
      for (var a = 0; a < added.length; a++) {
        if (isElement(added[a])) {
          scan(added[a]);
        }
      }
    }
    touched.forEach(function (el) {
      var record = records.get(el);
      if (record && el.isConnected && changedInPlace(el, record)) {
        remount(el);
      } else if (!record && el.isConnected && el.hasAttribute("data-svelte-island")) {
        setup(el); // An element became an island.
      }
    });
  }

  function warnMissing() {
    pending.forEach(function (el) {
      var record = records.get(el);
      if (record && record.ready && !registry.has(record.name)) {
        console.warn("autumn-svelte: no component is registered with the name:", record.name);
      }
    });
  }

  var queued = Array.isArray(existing) ? existing : [];
  window.autumnSvelte = {
    loader: true,
    // Registers `{ mount, unmount, components }` entries.
    push: function () {
      for (var i = 0; i < arguments.length; i++) {
        register(arguments[i]);
      }
      return registry.size;
    },
    // Mounts new islands in `root` (default: document). Safe to repeat.
    scan: scan,
  };
  queued.forEach(register);

  new MutationObserver(onMutations).observe(document.documentElement, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["data-svelte-island", "data-svelte-props", "data-svelte-mount", STATE],
  });

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      scan(document);
    });
  } else {
    scan(document);
  }

  if (document.readyState === "complete") {
    setTimeout(warnMissing, 0);
  } else {
    window.addEventListener("load", warnMissing);
  }
})();
