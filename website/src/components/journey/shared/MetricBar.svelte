<script lang="ts">
  import { onMount } from 'svelte';

  let {
    label,
    value,
    max = 10,
    color = 'var(--color-accent)',
    suffix = '',
    animated = true,
  }: {
    label: string;
    value: number;
    max?: number;
    color?: string;
    suffix?: string;
    animated?: boolean;
  } = $props();

  let mounted = $state(false);
  const pct = $derived((value / max) * 100);

  onMount(() => {
    if (animated) {
      requestAnimationFrame(() => { mounted = true; });
    } else {
      mounted = true;
    }
  });
</script>

<div class="metric-bar">
  <div class="bar-header">
    <span class="bar-label">{label}</span>
    <span class="bar-value" style="color: {color}">{value}{suffix}</span>
  </div>
  <div class="bar-track">
    <div
      class="bar-fill"
      style="width: {mounted ? pct : 0}%; background: {color}"
    ></div>
  </div>
</div>

<style>
  .metric-bar {
    margin-bottom: var(--space-2, 8px);
  }

  .bar-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: var(--space-1, 4px);
  }

  .bar-label {
    font-size: 0.75rem;
    color: var(--color-text-secondary, #aca9a3);
    text-transform: capitalize;
  }

  .bar-value {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 700;
  }

  .bar-track {
    height: 4px;
    background: var(--color-bg-tertiary, #242424);
    border-radius: 9999px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 9999px;
    transition: width 0.6s ease;
  }
</style>
