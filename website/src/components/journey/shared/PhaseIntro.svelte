<script lang="ts">
  let {
    intro,
    metrics = {} as Record<string, number | string>,
  }: {
    intro: string;
    metrics?: Record<string, number | string>;
  } = $props();

  const metricEntries = $derived(Object.entries(metrics));
</script>

<div class="phase-intro">
  {#if intro}
    <blockquote class="intro-quote">{intro}</blockquote>
  {/if}

  {#if metricEntries.length > 0}
    <div class="metric-chips">
      {#each metricEntries as [key, value]}
        <span class="metric-chip">
          <span class="metric-value">{value}</span>
          <span class="metric-label">{key}</span>
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .phase-intro {
    margin-bottom: var(--space-4, 16px);
  }

  .intro-quote {
    margin: 0 0 var(--space-4, 16px) 0;
    padding: var(--space-3, 12px) var(--space-4, 16px);
    border-left: 3px solid var(--color-accent, #7eb7a6);
    background: rgba(126, 183, 166, 0.05);
    border-radius: 0 var(--radius-sm, 4px) var(--radius-sm, 4px) 0;
    color: var(--color-text-secondary, #aca9a3);
    font-size: 0.8125rem;
    line-height: 1.6;
  }

  .metric-chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2, 8px);
  }

  .metric-chip {
    display: flex;
    align-items: center;
    gap: var(--space-1, 4px);
    padding: var(--space-1, 4px) var(--space-3, 12px);
    background: var(--color-bg-secondary, #161616);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-full, 9999px);
    font-size: 0.75rem;
  }

  .metric-value {
    font-weight: 700;
    font-family: var(--font-mono, monospace);
    color: var(--color-accent, #7eb7a6);
  }

  .metric-label {
    color: var(--color-text-muted, #908f8b);
  }
</style>
