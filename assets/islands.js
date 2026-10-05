/* autumn-plugin-svelte: island loader.
 *
 * Finds [data-svelte-island] elements and mounts Svelte components into them.
 * App bundles register components on a queue, so script order does not
 * matter:
 *
 *   (window.autumnSvelte = window.autumnSvelte || [])
 *     .push({ mount, unmount, components: { Counter } });
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
    console.error('autumn-svelte: island "' + record.name + '" failed to mount', error);
    emit(el, "autumn:svelte:error", { name: record.name, error: error });
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
    var props;
    try {
      props = parseProps(el);
    } catch (error) {
      fail(el, record, error);
      return;
    }
    var template = stashFallback(el);
    try {
      record.instance = entry.mount(entry.component, { target: el, props: props });
    } catch (error) {
      restoreFallback(el, template);
      fail(el, record, error);
      return;
    }
    record.entry = entry;
    record.state = "mounted";
    pending.delete(el);
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
    if (records.has(el) || !el.isConnected) {
      return;
    }
    if (el.hasAttribute(STATE)) {
      // Markup from an earlier mount (htmx history restore, a clone). Its
      // component DOM is stale; the fallback template is not.
      clearExceptFallback(el, fallbackOf(el));
      var stale = fallbackOf(el);
      if (stale) {
        restoreFallback(el, stale);
      }
    }
    var record = {
      name: el.getAttribute("data-svelte-island") || "",
      state: "pending",
      ready: false,
      cancel: null,
      instance: null,
      entry: null,
    };
    records.set(el, record);
    pending.add(el);
    el.setAttribute(STATE, "pending");
    var cancel = schedule(el, el.getAttribute("data-svelte-mount"));
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
    if (record.cancel) {
      record.cancel();
    }
    if (record.state !== "mounted") {
      return;
    }
    try {
      record.entry.unmount(record.instance);
    } catch (error) {
      console.error('autumn-svelte: island "' + record.name + '" failed to unmount', error);
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
        console.warn('autumn-svelte: component "' + name + '" is already registered; the first one stays');
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

  function onMutations(mutations) {
    for (var i = 0; i < mutations.length; i++) {
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
  }

  function warnMissing() {
    pending.forEach(function (el) {
      var record = records.get(el);
      if (record && record.ready && !registry.has(record.name)) {
        console.warn('autumn-svelte: no component "' + record.name + '" is registered');
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
