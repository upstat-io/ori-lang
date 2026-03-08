<script lang="ts">
  import type { JourneyData } from '../journey-types';
  import CodeBlock from '../shared/CodeBlock.svelte';

  let { data }: { data: JourneyData } = $props();

  const difficultyColor: Record<string, string> = {
    simple: 'var(--color-success, #7eb7a6)',
    moderate: 'var(--color-warning, #db844b)',
    complex: 'var(--color-error, #e84040)',
  };
</script>

<div class="source-phase">
  <div class="phase-header">
    <h2>Source Code</h2>
    <div class="meta-chips">
      <span class="chip" style="--chip-color: {difficultyColor[data.meta.difficulty] || '#aca9a3'}">
        {data.meta.difficulty}
      </span>
      {#each data.meta.features as feature}
        <span class="chip feature">{feature.replace(/_/g, ' ')}</span>
      {/each}
    </div>
  </div>

  {#if data.meta.featureDescription}
    <p class="description">{data.meta.featureDescription}</p>
  {/if}

  {#if data.meta.learningObjectives.length > 0}
    <div class="objectives">
      <h3>What you'll learn</h3>
      <ul>
        {#each data.meta.learningObjectives as obj}
          <li>{obj}</li>
        {/each}
      </ul>
    </div>
  {/if}

  <CodeBlock code={data.source} language="ori" />

  {#if data.executionResults.length > 0}
    <div class="exec-results">
      <h3>Execution Results</h3>
      <div class="results-grid">
        {#each data.executionResults as result}
          <div class="result-card">
            <span class="result-backend">{result.backend}</span>
            <span class="result-code">{result.exitCode}</span>
            <span class="result-badge" class:pass={result.status === 'PASS'} class:fail={result.status !== 'PASS'}>
              {result.status}
            </span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .source-phase {
    padding: var(--space-6, 24px);
  }

  .phase-header {
    margin-bottom: var(--space-4, 16px);
  }

  .phase-header h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-3, 12px) 0;
  }

  .meta-chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2, 8px);
  }

  .chip {
    display: inline-block;
    padding: 2px 10px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    background: color-mix(in srgb, var(--chip-color) 15%, transparent);
    color: var(--chip-color);
  }

  .chip.feature {
    --chip-color: var(--color-text-muted, #908f8b);
    text-transform: none;
    font-weight: 500;
  }

  .description {
    font-size: 0.875rem;
    color: var(--color-text-secondary, #aca9a3);
    line-height: 1.6;
    margin-bottom: var(--space-4, 16px);
  }

  .objectives {
    margin-bottom: var(--space-4, 16px);
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: rgba(126, 183, 166, 0.05);
    border: 1px solid rgba(126, 183, 166, 0.12);
    border-radius: var(--radius-md, 8px);
  }

  .objectives h3 {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--color-accent, #7eb7a6);
    margin: 0 0 var(--space-2, 8px) 0;
  }

  .objectives ul {
    margin: 0;
    padding-left: var(--space-5, 20px);
  }

  .objectives li {
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    line-height: 1.6;
    margin-bottom: var(--space-1, 4px);
  }

  .exec-results {
    margin-top: var(--space-6, 24px);
  }

  .exec-results h3 {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-3, 12px) 0;
  }

  .results-grid {
    display: flex;
    gap: var(--space-3, 12px);
    flex-wrap: wrap;
  }

  .result-card {
    display: flex;
    align-items: center;
    gap: var(--space-3, 12px);
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: var(--color-bg-secondary, #161616);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    min-width: 160px;
  }

  .result-backend {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--color-text-secondary, #aca9a3);
    text-transform: uppercase;
  }

  .result-code {
    font-family: var(--font-mono, monospace);
    font-size: 1.125rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
  }

  .result-badge {
    padding: 2px 8px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .result-badge.pass {
    background: rgba(126, 183, 166, 0.15);
    color: #7eb7a6;
  }

  .result-badge.fail {
    background: rgba(232, 64, 64, 0.15);
    color: #e84040;
  }
</style>
