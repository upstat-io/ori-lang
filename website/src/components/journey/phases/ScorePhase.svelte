<script lang="ts">
  import { onMount } from 'svelte';
  import type { ScoreBreakdown, Finding, ScrutinyCategory } from '../journey-types';
  import MetricBar from '../shared/MetricBar.svelte';
  import DataTable from '../shared/DataTable.svelte';

  let {
    score,
    findings,
    scrutiny,
    verdict,
  }: {
    score: { overall: number; breakdown: ScoreBreakdown[] };
    findings: Finding[];
    scrutiny: ScrutinyCategory[];
    verdict: string;
  } = $props();

  let ringAnimated = $state(false);
  let expandedFindings: Set<string> = $state(new Set());

  const scoreColor = $derived(
    score.overall >= 9 ? '#7eb7a6' :
    score.overall >= 7 ? '#569cd6' :
    score.overall >= 5 ? '#db844b' :
    '#e84040'
  );

  const dashArray = $derived(`${score.overall * 10}, 100`);

  function toggleFinding(id: string) {
    const next = new Set(expandedFindings);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    expandedFindings = next;
  }

  function scoreBarColor(value: number): string {
    if (value >= 9) return '#7eb7a6';
    if (value >= 7) return '#569cd6';
    if (value >= 5) return '#db844b';
    return '#e84040';
  }

  onMount(() => {
    requestAnimationFrame(() => {
      ringAnimated = true;
    });
  });
</script>

<div class="score-phase">
  <h2>Score</h2>

  <!-- Score ring -->
  <div class="score-hero">
    <div class="score-ring">
      <svg viewBox="0 0 36 36">
        <path class="ring-bg"
          d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831" />
        <path class="ring-fill"
          style="stroke: {scoreColor}; stroke-dasharray: {ringAnimated ? dashArray : '0, 100'}"
          d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831" />
      </svg>
      <span class="score-value" style="color: {scoreColor}">{score.overall}</span>
    </div>
    <span class="score-label">/ 10</span>
  </div>

  <!-- Category breakdown -->
  {#if score.breakdown.length > 0}
    <div class="breakdown-section">
      <h3>Category Breakdown</h3>
      {#each score.breakdown as cat}
        <MetricBar
          label="{cat.category} ({cat.weight}%)"
          value={cat.score}
          max={10}
          color={scoreBarColor(cat.score)}
          suffix="/10"
        />
      {/each}
    </div>
  {/if}

  <!-- Findings -->
  {#if findings.length > 0}
    <div class="findings-section">
      <h3>Findings ({findings.length})</h3>
      <div class="findings-list">
        {#each findings as finding}
          <div class="finding-item">
            <button class="finding-header" onclick={() => toggleFinding(finding.id)}>
              <span class="finding-severity severity-{finding.severity.toLowerCase()}">{finding.severity}</span>
              <span class="finding-id">#{finding.id}</span>
              <span class="finding-desc">{finding.description}</span>
              <span class="finding-status">{finding.status}</span>
              <span class="finding-expand">{expandedFindings.has(finding.id) ? '\u25B4' : '\u25BE'}</span>
            </button>
            {#if expandedFindings.has(finding.id) && finding.detail}
              <div class="finding-detail">
                <pre>{finding.detail}</pre>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Scrutiny categories -->
  {#if scrutiny.length > 0}
    <div class="scrutiny-section">
      <h3>Deep Scrutiny</h3>
      {#each scrutiny as category}
        <details class="scrutiny-cat">
          <summary>{category.name}</summary>
          <div class="scrutiny-content">
            {#if category.table.length > 1}
              <DataTable
                headers={category.table[0]}
                rows={category.table.slice(1)}
              />
            {/if}
            {#if category.prose}
              <div class="scrutiny-prose">{category.prose}</div>
            {/if}
          </div>
        </details>
      {/each}
    </div>
  {/if}

  <!-- Verdict -->
  {#if verdict}
    <div class="verdict-section">
      <h3>Verdict</h3>
      <blockquote class="verdict-text">{verdict}</blockquote>
    </div>
  {/if}
</div>

<style>
  .score-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-3, 12px) 0;
  }

  /* Score ring */
  .score-hero {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2, 8px);
    margin-bottom: var(--space-6, 24px);
  }

  .score-ring {
    position: relative;
    width: 120px;
    height: 120px;
  }

  .score-ring svg {
    width: 120px;
    height: 120px;
    transform: rotate(-90deg);
  }

  .ring-bg {
    fill: none;
    stroke: var(--color-bg-tertiary, #242424);
    stroke-width: 3;
  }

  .ring-fill {
    fill: none;
    stroke-width: 3;
    stroke-linecap: round;
    transition: stroke-dasharray 1s ease;
  }

  .score-value {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono, monospace);
    font-size: 1.75rem;
    font-weight: 700;
  }

  .score-label {
    font-size: 0.875rem;
    color: var(--color-text-muted, #908f8b);
  }

  /* Breakdown */
  .breakdown-section {
    margin-bottom: var(--space-6, 24px);
    padding: var(--space-4, 16px);
    background: var(--color-bg-secondary, #161616);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
  }

  /* Findings */
  .findings-section {
    margin-bottom: var(--space-6, 24px);
  }

  .findings-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    overflow: hidden;
  }

  .finding-item {
    background: var(--color-bg-secondary, #161616);
  }

  .finding-header {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
    width: 100%;
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    font-size: 0.8125rem;
    transition: background 0.1s ease;
  }

  .finding-header:hover {
    background: var(--color-bg-tertiary, #242424);
  }

  .finding-severity {
    padding: 1px 8px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.5625rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .finding-severity.severity-critical { background: rgba(232, 64, 64, 0.15); color: #e84040; }
  .finding-severity.severity-high     { background: rgba(219, 132, 75, 0.15); color: #db844b; }
  .finding-severity.severity-medium   { background: rgba(219, 132, 75, 0.15); color: #db844b; }
  .finding-severity.severity-low      { background: rgba(139, 168, 196, 0.15); color: #8ba8c4; }
  .finding-severity.severity-note     { background: rgba(164, 161, 157, 0.1); color: #aca9a3; }

  .finding-id {
    font-family: var(--font-mono, monospace);
    font-size: 0.6875rem;
    color: var(--color-text-muted, #908f8b);
    flex-shrink: 0;
  }

  .finding-desc {
    flex: 1;
    color: var(--color-text-secondary, #aca9a3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .finding-status {
    font-size: 0.625rem;
    font-weight: 600;
    color: var(--color-text-muted, #908f8b);
    flex-shrink: 0;
  }

  .finding-expand {
    color: var(--color-text-muted, #908f8b);
    font-size: 0.625rem;
    flex-shrink: 0;
  }

  .finding-detail {
    padding: var(--space-3, 12px) var(--space-4, 16px);
    border-top: 1px solid var(--color-border, #333333);
    background: var(--color-bg-code, #111111);
  }

  .finding-detail pre {
    margin: 0;
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    line-height: 1.6;
    color: var(--color-text-secondary, #aca9a3);
    white-space: pre-wrap;
  }

  /* Scrutiny */
  .scrutiny-section {
    margin-bottom: var(--space-6, 24px);
  }

  .scrutiny-cat {
    margin-bottom: var(--space-2, 8px);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    overflow: hidden;
  }

  .scrutiny-cat summary {
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: var(--color-bg-secondary, #161616);
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--color-text-secondary, #aca9a3);
    cursor: pointer;
    transition: background 0.1s ease;
  }

  .scrutiny-cat summary:hover {
    background: var(--color-bg-tertiary, #242424);
  }

  .scrutiny-content {
    padding: var(--space-3, 12px);
  }

  .scrutiny-prose {
    margin-top: var(--space-3, 12px);
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    line-height: 1.6;
    white-space: pre-wrap;
  }

  /* Verdict */
  .verdict-section {
    margin-bottom: var(--space-6, 24px);
  }

  .verdict-text {
    margin: 0;
    padding: var(--space-4, 16px);
    border-left: 3px solid var(--color-accent, #7eb7a6);
    background: rgba(126, 183, 166, 0.05);
    border-radius: 0 var(--radius-md, 8px) var(--radius-md, 8px) 0;
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    line-height: 1.7;
  }
</style>
