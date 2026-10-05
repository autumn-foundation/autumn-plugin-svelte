<script>
  // Props come from `Island::props` on the server.
  let { start = 0, label = "Count" } = $props();
  // `start` is only the first value. Later prop changes do not reset it.
  // svelte-ignore state_referenced_locally
  let count = $state(start);
</script>

<div class="counter">
  <button type="button" onclick={() => count--} aria-label="Decrease">-</button>
  <output aria-live="polite">{label}: {count}</output>
  <button type="button" onclick={() => count++} aria-label="Increase">+</button>
</div>

<style>
  .counter {
    display: inline-flex;
    gap: 0.5rem;
    align-items: center;
  }
  button {
    min-width: 2rem;
  }
</style>
