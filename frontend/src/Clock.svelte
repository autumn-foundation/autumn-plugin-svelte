<script>
  // A component with a timer. The loader unmounts it when htmx removes it,
  // so the timer stops.
  let now = $state(new Date());

  $effect(() => {
    const timer = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(timer);
  });
</script>

<time class="clock" datetime={now.toISOString()}>{now.toLocaleTimeString()}</time>

<style>
  .clock {
    font-variant-numeric: tabular-nums;
  }
</style>
