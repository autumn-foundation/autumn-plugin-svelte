/* Test bundle. It keeps the loader contract with fake components, and it
 * writes each lifecycle step to window.__svelteLog. */
(function () {
  "use strict";
  var log = (window.__svelteLog = window.__svelteLog || []);
  ["autumn:svelte:mount", "autumn:svelte:unmount", "autumn:svelte:error"].forEach(function (type) {
    document.addEventListener(type, function (event) {
      log.push(type + ":" + event.detail.name);
    });
  });

  function mount(Component, options) {
    return Component(options.target, options.props);
  }

  function unmount(instance) {
    instance.destroy();
  }

  // Shows its props as JSON.
  function Echo(target, props) {
    var key = JSON.stringify(props);
    var span = document.createElement("span");
    span.className = "echo";
    span.textContent = key;
    target.appendChild(span);
    log.push("mount:" + key);
    return {
      destroy: function () {
        span.remove();
        log.push("destroy:" + key);
      },
    };
  }

  // Fails before it adds DOM.
  function Boom() {
    throw new Error("boom");
  }

  // Adds DOM, then fails.
  function Partial(target) {
    var b = document.createElement("b");
    b.textContent = "partial DOM";
    target.appendChild(b);
    throw new Error("partial");
  }

  (window.autumnSvelte = window.autumnSvelte || []).push({
    mount: mount,
    unmount: unmount,
    components: { Echo: Echo, Boom: Boom, Partial: Partial },
  });
})();
