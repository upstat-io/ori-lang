<script lang="ts">
  import type { RunResult } from '../playground/types';

  let {
    result = null,
    elapsed = '',
    status = 'idle',
    validated = null,
    hints = [],
    expectedOutput = '',
    outputMatch = 'none',
  }: {
    result: RunResult | null;
    elapsed: string;
    status: 'idle' | 'running' | 'success' | 'error';
    validated: boolean | null;
    hints: string[];
    expectedOutput: string;
    outputMatch: string;
  } = $props();

  const actualOutput = $derived.by(() => {
    if (!result) return '';
    if (result.success) {
      let out = '';
      if (result.printed) out += result.printed;
      if (result.output) {
        if (out) out += '\n';
        out += result.output;
      }
      return out;
    }
    return result.error || 'Unknown error';
  });

  const statusLabel = $derived(
    status === 'running' ? 'Running...'
    : validated === true ? 'Correct!'
    : validated === false ? 'Try Again'
    : status === 'success' ? 'Ran'
    : status === 'error' ? (
      result?.error_type === 'parse' ? 'Parse Error'
      : result?.error_type === 'type' ? 'Type Error'
      : 'Runtime Error'
    )
    : ''
  );

  const statusClass = $derived(
    validated === true ? 'validated'
    : validated === false ? 'failed'
    : status
  );
</script>

<div class="output-pane">
  <div class="pane-header">
    <span>Output</span>
    {#if statusLabel}
      <span class="status {statusClass}">{statusLabel}</span>
    {/if}
  </div>
  <div class="output-body">
    {#if validated === true}
      <div class="validation-banner success-banner">
        <svg width="20" height="20" viewBox="0 0 16 16" fill="currentColor">
          <path d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"/>
        </svg>
        <span>Lesson complete! Move on to the next lesson.</span>
      </div>
    {:else if validated === false && outputMatch !== 'none'}
      <div class="validation-banner fail-banner">
        <div class="expected-vs-actual">
          <div class="compare-block">
            <span class="compare-label">Expected:</span>
            <code class="compare-value">{expectedOutput}</code>
          </div>
          <div class="compare-block">
            <span class="compare-label">Got:</span>
            <code class="compare-value got">{actualOutput || '(no output)'}</code>
          </div>
        </div>
        {#if hints.length > 0}
          <div class="hints">
            {#each hints as hint}
              <div class="hint">{hint}</div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    <pre class="output" class:error={status === 'error'} class:success={status === 'success'}>{actualOutput || (result ? '(no output)' : '')}</pre>

    {#if elapsed}
      <div class="timing-line">
        <span class="timing-duration">Ran in {elapsed}</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .output-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    flex: 1;
  }

  .pane-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--color-bg-elevated, #282828);
    border-bottom: 1px solid var(--color-border, #333333);
    border-top: 1px solid var(--color-border, #333333);
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    flex-shrink: 0;
  }

  .output-body {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .output {
    flex: 1;
    margin: 0;
    padding: 0.75rem 1rem;
    font-family: var(--font-mono, 'JetBrains Mono', monospace);
    font-size: 0.8125rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
    color: var(--color-text-secondary, #aca9a3);
    background: transparent;
  }

  .output.error {
    color: var(--color-error, #e84040);
  }

  .output.success {
    color: var(--color-text-primary, #cccbc7);
  }

  .status {
    font-size: 0.6875rem;
    padding: 0.125rem 0.5rem;
    border-radius: 3px;
    font-weight: 600;
  }

  .status.running {
    background: var(--color-warning, #db844b);
    color: #000;
  }

  .status.success,
  .status.validated {
    background: var(--color-success, #6ba591);
    color: #000;
  }

  .status.error,
  .status.failed {
    background: var(--color-error, #e84040);
    color: #fff;
  }

  /* Validation banners */
  .validation-banner {
    padding: 0.75rem 1rem;
    font-size: 0.8125rem;
    flex-shrink: 0;
  }

  .success-banner {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: rgba(126, 183, 166, 0.1);
    border-bottom: 1px solid rgba(126, 183, 166, 0.3);
    color: var(--color-success, #6ba591);
  }

  .fail-banner {
    background: rgba(241, 76, 76, 0.05);
    border-bottom: 1px solid rgba(241, 76, 76, 0.2);
  }

  .expected-vs-actual {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .compare-block {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }

  .compare-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--color-text-muted, #908f8b);
    flex-shrink: 0;
    min-width: 4.5rem;
  }

  .compare-value {
    font-family: var(--font-mono, 'JetBrains Mono', monospace);
    font-size: 0.8125rem;
    padding: 0.125rem 0.375rem;
    background: var(--color-bg-tertiary, #242424);
    border-radius: 3px;
    color: var(--color-success, #6ba591);
  }

  .compare-value.got {
    color: var(--color-error, #e84040);
    background: var(--color-bg-secondary, #161616);
  }

  .hints {
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--color-border, #333333);
  }

  .hint {
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    padding: 0.125rem 0;
  }

  .hint::before {
    content: '\1F4A1 ';
  }

  .timing-line {
    padding: 0.375rem 1rem 0.75rem;
    font-size: 0.75rem;
    color: var(--color-text-muted, #908f8b);
    flex-shrink: 0;
  }

  .timing-duration {
    color: var(--color-success, #6ba591);
  }
</style>
