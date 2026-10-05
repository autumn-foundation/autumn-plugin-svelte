/* Test probe. It loads before the loader. It holds idle callbacks until the
 * test runs them, and it counts IntersectionObserver disconnects. */
(function () {
  "use strict";
  var queue = [];
  window.__idleCancels = 0;
  window.requestIdleCallback = function (callback) {
    queue.push(callback);
    return queue.length;
  };
  window.cancelIdleCallback = function (id) {
    window.__idleCancels++;
    queue[id - 1] = null;
  };
  window.__runIdle = function () {
    var callbacks = queue;
    queue = [];
    callbacks.forEach(function (callback) {
      if (callback) {
        callback({ didTimeout: false, timeRemaining: function () { return 50; } });
      }
    });
  };
  var disconnect = IntersectionObserver.prototype.disconnect;
  window.__disconnects = 0;
  IntersectionObserver.prototype.disconnect = function () {
    window.__disconnects++;
    return disconnect.apply(this, arguments);
  };
})();
