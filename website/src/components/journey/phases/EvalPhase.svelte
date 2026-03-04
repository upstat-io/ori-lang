<script lang="ts">
  import type { InterpreterData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';

  let { interpreter }: { interpreter: InterpreterData } = $props();

  let visibleLines = $state(0);
  let showAll = $state(false);

  const traceLines = $derived(interpreter.trace.split('\n'));
  const displayCount = $derived(showAll ? traceLines.length : visibleLines);

  function stepNext() {
    if (visibleLines < traceLines.length) {
      visibleLines++;
    }
  }

  function runAll() {
    showAll = true;
    visibleLines = traceLines.length;
  }

  function reset() {
    visibleLines = 0;
    showAll = false;
  }
</script>

<div class="eval-phase">
  <h2>Interpreter (Eval)</h2>

  <PhaseIntro intro={interpreter.intro} />

  <div class="result-row">
    <span class="result-label">Result:</span>
    <span class="result-value">{interpreter.result}</span>
    <span class="status-badge" class:pass={interpreter.status === 'PASS'} class:fail={interpreter.status !== 'PASS'}>
      {interpreter.status}
    </span>
  </div>

  <div class="trace-controls">
    <button onclick={stepNext} disabled={visibleLines >= traceLines.length}>
      Next Step
    </button>
    <button onclick={runAll} disabled={showAll}>
      Run All
    </button>
    <button onclick={reset} disabled={visibleLines === 0}>
      Reset
    </button>
    <span class="step-counter">
      {displayCount} / {traceLines.length} steps
    </span>
  </div>

  <div class="trace-view">
    <pre><code>{#each traceLines.slice(0, displayCount) as line, i}<span
        class="trace-line"
        class:current={!showAll && i === displayCount - 1}
        class:result-line={line.startsWith('\u2192') || line.startsWith('→')}
      >{line}
</span>{/each}</code></pre>
  </div>
</div>

<style>
  .eval-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #e5e7eb);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  .result-row {
    display: flex;
    align-items: center;
    gap: var(--space-3, 12px);
    margin-bottom: var(--space-4, 16px);
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: var(--color-bg-secondary, #13141a);
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-md, 8px);
  }

  .result-label {
    font-size: 0.8125rem;
    color: var(--color-text-muted, #636874);
  }

  .result-value {
    font-family: var(--font-mono, monospace);
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #e5e7eb);
  }

  .status-badge {
    padding: 2px 10px;
    border-radius: var(--radius-full, 9999px);
    font-size: 0.625rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .status-badge.pass {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
  }

  .status-badge.fail {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  .trace-controls {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
    margin-bottom: var(--space-3, 12px);
  }

  .trace-controls button {
    padding: var(--space-1, 4px) var(--space-3, 12px);
    background: var(--color-bg-tertiary, #1a1b23);
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-md, 8px);
    color: var(--color-text-primary, #e5e7eb);
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .trace-controls button:hover:not(:disabled) {
    background: var(--color-bg-elevated, #22232d);
    border-color: var(--color-accent, #569cd6);
  }

  .trace-controls button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .step-counter {
    font-size: 0.6875rem;
    color: var(--color-text-muted, #636874);
    font-family: var(--font-mono, monospace);
    margin-left: auto;
  }

  .trace-view {
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-md, 8px);
    background: var(--color-bg-code, #0d0e12);
    overflow: auto;
    max-height: 500px;
  }

  pre {
    margin: 0;
    padding: var(--space-3, 12px);
    font-family: var(--font-mono, monospace);
    font-size: 0.8125rem;
    line-height: 1.6;
  }

  .trace-line {
    display: block;
    padding: 1px var(--space-2, 8px);
    border-radius: var(--radius-sm, 4px);
    color: var(--color-text-secondary, #9ca3af);
    transition: background 0.15s ease;
  }

  .trace-line.current {
    background: rgba(86, 156, 214, 0.12);
    color: var(--color-text-primary, #e5e7eb);
  }

  .trace-line.result-line {
    color: #4ec9b0;
    font-weight: 600;
  }
</style>
