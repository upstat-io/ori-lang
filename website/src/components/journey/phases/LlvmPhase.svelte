<script lang="ts">
  import type { LlvmData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';
  import CodeBlock from '../shared/CodeBlock.svelte';
  import DataTable from '../shared/DataTable.svelte';

  let { llvm }: { llvm: LlvmData } = $props();

  let activeTab: 'ir' | 'ideal' | 'asm' = $state('ir');

  const binaryMetricEntries = $derived(Object.entries(llvm.binaryMetrics));
</script>

<div class="llvm-phase">
  <h2>LLVM Codegen</h2>

  <PhaseIntro intro={llvm.intro} />

  <div class="tab-bar">
    <button class:active={activeTab === 'ir'} onclick={() => activeTab = 'ir'}>
      LLVM IR
    </button>
    <button class:active={activeTab === 'ideal'} onclick={() => activeTab = 'ideal'}>
      Ideal vs Actual
    </button>
    <button class:active={activeTab === 'asm'} onclick={() => activeTab = 'asm'}>
      Assembly
    </button>
  </div>

  <div class="tab-content">
    {#if activeTab === 'ir'}
      <CodeBlock code={llvm.ir || 'No LLVM IR available'} language="llvm" />
    {:else if activeTab === 'ideal'}
      {#if llvm.idealVsActual.length > 0}
        <div class="comparisons">
          {#each llvm.idealVsActual as cmp}
            <div class="comparison-card">
              <div class="cmp-header">
                <span class="cmp-fn">{cmp.fn}</span>
                <div class="cmp-stats">
                  <span class="cmp-count">Ideal: {cmp.idealCount}</span>
                  <span class="cmp-count">Actual: {cmp.actualCount}</span>
                  <span class="cmp-delta" class:optimal={cmp.idealCount === cmp.actualCount}>
                    +{cmp.actualCount - cmp.idealCount}
                  </span>
                  <span class="cmp-verdict"
                    class:verdict-optimal={cmp.verdict.includes('OPTIMAL') && !cmp.verdict.includes('NEAR')}
                    class:verdict-near={cmp.verdict.includes('NEAR')}
                  >
                    {cmp.verdict}
                  </span>
                </div>
              </div>

              {#if cmp.ideal && cmp.actual}
                <div class="cmp-panels">
                  <div class="cmp-panel">
                    <span class="panel-label">Ideal</span>
                    <CodeBlock code={cmp.ideal} language="llvm" showLineNumbers={false} />
                  </div>
                  <div class="cmp-panel">
                    <span class="panel-label">Actual</span>
                    <CodeBlock code={cmp.actual} language="llvm" showLineNumbers={false} />
                  </div>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <p class="empty-state">No ideal vs actual comparison available.</p>
      {/if}
    {:else if activeTab === 'asm'}
      <CodeBlock code={llvm.assembly || 'No assembly available'} language="asm" />

      {#if binaryMetricEntries.length > 0}
        <div class="binary-metrics">
          <h3>Binary Metrics</h3>
          <DataTable
            headers={['Metric', 'Value']}
            rows={binaryMetricEntries.map(([k, v]) => [k, v])}
          />
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .llvm-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #e5e7eb);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-primary, #e5e7eb);
    margin: var(--space-4, 16px) 0 var(--space-3, 12px) 0;
  }

  .tab-bar {
    display: flex;
    gap: 2px;
    margin-bottom: var(--space-4, 16px);
    background: var(--color-bg-tertiary, #1a1b23);
    border-radius: var(--radius-md, 8px);
    padding: 2px;
    width: fit-content;
  }

  .tab-bar button {
    padding: var(--space-2, 8px) var(--space-4, 16px);
    background: none;
    border: none;
    border-radius: var(--radius-sm, 4px);
    color: var(--color-text-muted, #636874);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .tab-bar button.active {
    background: var(--color-bg-secondary, #13141a);
    color: var(--color-text-primary, #e5e7eb);
  }

  .tab-bar button:hover:not(.active) {
    color: var(--color-text-secondary, #9ca3af);
  }

  .comparisons {
    display: flex;
    flex-direction: column;
    gap: var(--space-4, 16px);
  }

  .comparison-card {
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-md, 8px);
    overflow: hidden;
  }

  .cmp-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: var(--color-bg-secondary, #13141a);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    flex-wrap: wrap;
    gap: var(--space-2, 8px);
  }

  .cmp-fn {
    font-family: var(--font-mono, monospace);
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-accent, #569cd6);
  }

  .cmp-stats {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
  }

  .cmp-count {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #636874);
    font-family: var(--font-mono, monospace);
  }

  .cmp-delta {
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--color-warning, #fbbf24);
  }

  .cmp-delta.optimal {
    color: var(--color-success, #4ade80);
  }

  .cmp-verdict {
    padding: 1px 8px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 700;
  }

  .cmp-verdict.verdict-optimal {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
  }

  .cmp-verdict.verdict-near {
    background: rgba(251, 191, 36, 0.12);
    color: #fbbf24;
  }

  .cmp-panels {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1px;
    background: var(--color-border, #2a2b35);
  }

  .cmp-panel {
    background: var(--color-bg-code, #0d0e12);
  }

  .panel-label {
    display: block;
    padding: var(--space-1, 4px) var(--space-3, 12px);
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #636874);
    background: var(--color-bg-tertiary, #1a1b23);
    border-bottom: 1px solid var(--color-border, #2a2b35);
  }

  .empty-state {
    text-align: center;
    padding: var(--space-8, 32px);
    color: var(--color-text-muted, #636874);
    font-size: 0.8125rem;
  }

  .binary-metrics {
    margin-top: var(--space-4, 16px);
  }

  @media (max-width: 768px) {
    .cmp-panels {
      grid-template-columns: 1fr;
    }
  }
</style>
