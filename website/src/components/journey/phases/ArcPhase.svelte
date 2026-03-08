<script lang="ts">
  import type { ArcData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';

  let { arc }: { arc: ArcData } = $props();

  const totalOps = $derived(
    arc.functions.reduce((sum, f) => sum + f.rcInc + f.rcDec, 0)
  );
  const allBalanced = $derived(arc.functions.every(f => f.balanced));
</script>

<div class="arc-phase">
  <h2>ARC Pipeline</h2>

  <PhaseIntro intro={arc.intro} metrics={arc.metrics} />

  {#if totalOps === 0}
    <div class="optimal-badge">
      <span class="badge-icon">&#x2713;</span>
      <span class="badge-text">Zero RC operations — optimal for scalar-only program</span>
    </div>
  {/if}

  <div class="function-cards">
    {#each arc.functions as fn}
      <div class="fn-card" class:balanced={fn.balanced} class:unbalanced={!fn.balanced}>
        <div class="fn-header">
          <span class="fn-name">{fn.name}</span>
          <span class="balance-badge" class:pass={fn.balanced} class:fail={!fn.balanced}>
            {fn.balanced ? 'Balanced' : 'Unbalanced'}
          </span>
        </div>
        <div class="fn-metrics">
          <div class="rc-stat">
            <span class="rc-label">rc_inc</span>
            <span class="rc-value">{fn.rcInc}</span>
          </div>
          <div class="rc-stat">
            <span class="rc-label">rc_dec</span>
            <span class="rc-value">{fn.rcDec}</span>
          </div>
        </div>
        {#if fn.notes}
          <p class="fn-notes">{fn.notes}</p>
        {/if}
      </div>
    {/each}
  </div>

  {#if arc.annotations}
    <details class="raw-annotations">
      <summary>Raw ARC annotations</summary>
      <pre><code>{arc.annotations}</code></pre>
    </details>
  {/if}
</div>

<style>
  .arc-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  .optimal-badge {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: rgba(126, 183, 166, 0.08);
    border: 1px solid rgba(126, 183, 166, 0.2);
    border-radius: var(--radius-md, 8px);
    margin-bottom: var(--space-4, 16px);
  }

  .badge-icon {
    color: #7eb7a6;
    font-size: 1rem;
    font-weight: 700;
  }

  .badge-text {
    font-size: 0.8125rem;
    color: #7eb7a6;
    font-weight: 500;
  }

  .function-cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: var(--space-3, 12px);
    margin-bottom: var(--space-4, 16px);
  }

  .fn-card {
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: var(--color-bg-secondary, #161616);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
  }

  .fn-card.balanced {
    border-color: rgba(126, 183, 166, 0.2);
  }

  .fn-card.unbalanced {
    border-color: rgba(232, 64, 64, 0.3);
  }

  .fn-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--space-2, 8px);
  }

  .fn-name {
    font-family: var(--font-mono, monospace);
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-accent, #7eb7a6);
  }

  .balance-badge {
    padding: 1px 8px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 700;
  }

  .balance-badge.pass {
    background: rgba(126, 183, 166, 0.15);
    color: #7eb7a6;
  }

  .balance-badge.fail {
    background: rgba(232, 64, 64, 0.15);
    color: #e84040;
  }

  .fn-metrics {
    display: flex;
    gap: var(--space-4, 16px);
  }

  .rc-stat {
    display: flex;
    align-items: baseline;
    gap: var(--space-1, 4px);
  }

  .rc-label {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #908f8b);
  }

  .rc-value {
    font-family: var(--font-mono, monospace);
    font-size: 0.875rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
  }

  .fn-notes {
    margin: var(--space-2, 8px) 0 0;
    font-size: 0.75rem;
    color: var(--color-text-muted, #908f8b);
    font-style: italic;
  }

  .raw-annotations {
    margin-top: var(--space-4, 16px);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
  }

  .raw-annotations summary {
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: var(--color-bg-secondary, #161616);
    font-size: 0.75rem;
    color: var(--color-text-muted, #908f8b);
    cursor: pointer;
  }

  .raw-annotations pre {
    margin: 0;
    padding: var(--space-3, 12px);
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    line-height: 1.5;
    overflow-x: auto;
    color: var(--color-text-secondary, #aca9a3);
  }
</style>
