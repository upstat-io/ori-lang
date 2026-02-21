<script lang="ts">
  import type { RunResult } from './types';

  let {
    result = null,
    elapsed = '',
    status = 'idle',
  }: {
    result: RunResult | null;
    elapsed: string;
    status: 'idle' | 'running' | 'success' | 'error';
  } = $props();

  const statusLabel = $derived(
    status === 'running' ? 'Running...'
    : status === 'success' ? 'Success'
    : status === 'error' ? (
      result?.error_type === 'parse' ? 'Parse Error'
      : result?.error_type === 'type' ? 'Type Error'
      : 'Runtime Error'
    )
    : ''
  );

  const outputText = $derived.by(() => {
    if (!result) return '';
    if (result.success) {
      let out = '';
      if (result.printed) out += result.printed;
      if (result.output) {
        if (out) out += '\n';
        out += result.output;
      }
      return out || '(no output)';
    }
    return result.error || 'Unknown error';
  });
</script>

<div class="output-pane">
  <div class="pane-header">
    <span>Output</span>
    {#if statusLabel}
      <span class="status {status}">{statusLabel}</span>
    {/if}
  </div>
  <div class="output" class:error={status === 'error'} class:success={status === 'success'}>
    {outputText}
    {#if elapsed}
      <div class="timing-line">
        <span class="timing-duration">Ran in {elapsed}</span>
        &middot;
        <span class="timing-label">interpreted in WASM</span>
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
    background: var(--color-bg-elevated, #21222c);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #9ca0ab);
    flex-shrink: 0;
  }

  .output {
    flex: 1;
    padding: 1rem;
    overflow: auto;
    font-family: var(--font-mono, 'JetBrains Mono', monospace);
    font-size: 0.875rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-wrap: break-word;
    color: var(--color-text-secondary, #9ca0ab);
  }

  .output.error {
    color: var(--color-error, #f14c4c);
  }

  .output.success {
    color: var(--color-text-primary, #e2e4e9);
  }

  .status {
    font-size: 0.75rem;
    padding: 0.125rem 0.5rem;
    border-radius: 3px;
  }

  .status.running {
    background: var(--color-warning, #cca700);
    color: #000;
  }

  .status.success {
    background: var(--color-success, #4ec9b0);
    color: #000;
  }

  .status.error {
    background: var(--color-error, #f14c4c);
    color: #fff;
  }

  .timing-line {
    margin-top: 1rem;
    font-size: 0.8125rem;
    color: var(--color-text-muted, #636874);
  }

  .timing-duration {
    color: var(--color-success, #4ec9b0);
  }

  .timing-label {
    color: var(--color-text-muted, #636874);
  }
</style>
