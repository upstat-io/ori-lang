<script lang="ts">
  import { EXAMPLES } from './examples';

  let {
    enableShare = true,
    enableExamples = true,
    running = false,
    selectedExample = '',
    onrun,
    onformat,
    onshare,
    onexample,
    shareLabel = 'Share',
  }: {
    enableShare?: boolean;
    enableExamples?: boolean;
    running?: boolean;
    selectedExample?: string;
    onrun?: () => void;
    onformat?: () => void;
    onshare?: () => void;
    onexample?: (name: string) => void;
    shareLabel?: string;
  } = $props();
</script>

<div class="toolbar">
  <div class="toolbar-left">
    <span class="file-label">main.ori</span>
  </div>
  <div class="toolbar-right">
    <button class="btn btn-outline" disabled={running} onclick={onformat}>
      Format
    </button>
    <button class="btn btn-primary" disabled={running} onclick={onrun}>
      <span class="btn-icon">&#9654;</span>
      Run
    </button>
    {#if enableShare}
      <button class="btn btn-secondary" onclick={onshare}>{shareLabel}</button>
    {/if}
    {#if enableExamples}
      <select
        class="select"
        value={selectedExample}
        onchange={(e) => {
          const target = e.target as HTMLSelectElement;
          onexample?.(target.value);
        }}
      >
        <option value="">Examples...</option>
        {#each Object.entries(EXAMPLES) as [key, example]}
          <option value={key}>{example.label}</option>
        {/each}
      </select>
    {/if}
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--color-bg-elevated, #21222c);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
  }

  .file-label {
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #9ca0ab);
  }

  .toolbar-right {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .btn {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .btn-primary {
    background: var(--color-accent, #569cd6);
    color: #fff;
  }

  .btn-primary:hover {
    background: var(--color-accent-hover, #6cb6ff);
  }

  .btn-primary:disabled {
    background: var(--color-text-muted, #636874);
    cursor: not-allowed;
  }

  .btn-secondary {
    background: var(--color-bg-tertiary, #1a1b23);
    color: var(--color-text-primary, #e2e4e9);
    border: 1px solid var(--color-border, #2a2b35);
  }

  .btn-secondary:hover {
    background: var(--color-border-hover, #3c3d4a);
  }

  .btn-outline {
    background: transparent;
    color: var(--color-accent, #569cd6);
    border: 1px solid var(--color-accent, #569cd6);
  }

  .btn-outline:hover {
    background: rgba(86, 156, 214, 0.1);
  }

  .btn-outline:disabled {
    color: var(--color-text-muted, #636874);
    border-color: var(--color-text-muted, #636874);
    cursor: not-allowed;
  }

  .btn-icon {
    font-size: 0.75rem;
  }

  .select {
    padding: 0.5rem 0.75rem;
    background: var(--color-bg-tertiary, #1a1b23);
    color: var(--color-text-primary, #e2e4e9);
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.875rem;
    cursor: pointer;
  }

  .select:hover {
    border-color: var(--color-text-muted, #636874);
  }
</style>
