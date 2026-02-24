<script lang="ts">
  let {
    running = false,
    hasNext = false,
    hasPrev = false,
    lessonTitle = '',
    onrun,
    onformat,
    onreset,
    onsolution,
    onnext,
    onprev,
  }: {
    running?: boolean;
    hasNext?: boolean;
    hasPrev?: boolean;
    lessonTitle?: string;
    onrun?: () => void;
    onformat?: () => void;
    onreset?: () => void;
    onsolution?: () => void;
    onnext?: () => void;
    onprev?: () => void;
  } = $props();
</script>

<div class="toolbar">
  <div class="toolbar-left">
    <button class="btn btn-nav" disabled={!hasPrev} onclick={onprev} aria-label="Previous lesson">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
        <path d="M10 4L6 8L10 12" />
      </svg>
      Prev
    </button>
    <span class="lesson-label">{lessonTitle}</span>
    <button class="btn btn-nav" disabled={!hasNext} onclick={onnext} aria-label="Next lesson">
      Next
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
        <path d="M6 4L10 8L6 12" />
      </svg>
    </button>
  </div>
  <div class="toolbar-right">
    <button class="btn btn-ghost" onclick={onreset}>Reset</button>
    <button class="btn btn-ghost" onclick={onsolution}>Solution</button>
    <button class="btn btn-outline" disabled={running} onclick={onformat}>Format</button>
    <button class="btn btn-primary" disabled={running} onclick={onrun}>
      <span class="btn-icon">&#9654;</span>
      Run
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    background: var(--color-bg-elevated, #21222c);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    flex-shrink: 0;
    gap: 0.5rem;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .toolbar-right {
    display: flex;
    gap: 0.375rem;
    align-items: center;
    flex-shrink: 0;
  }

  .lesson-label {
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #9ca0ab);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.375rem 0.75rem;
    border: none;
    border-radius: 4px;
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
    white-space: nowrap;
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

  .btn-ghost {
    background: transparent;
    color: var(--color-text-secondary, #9ca0ab);
  }

  .btn-ghost:hover {
    color: var(--color-text-primary, #e2e4e9);
    background: var(--color-bg-tertiary, #1a1b23);
  }

  .btn-nav {
    background: transparent;
    color: var(--color-text-secondary, #9ca0ab);
    padding: 0.375rem 0.5rem;
  }

  .btn-nav:hover:not(:disabled) {
    color: var(--color-text-primary, #e2e4e9);
    background: var(--color-bg-tertiary, #1a1b23);
  }

  .btn-nav:disabled {
    color: var(--color-text-muted, #636874);
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-icon {
    font-size: 0.6875rem;
  }

  @media (max-width: 640px) {
    .toolbar {
      flex-wrap: wrap;
      gap: 0.375rem;
    }

    .toolbar-left {
      order: 2;
      width: 100%;
      justify-content: center;
    }

    .toolbar-right {
      order: 1;
      width: 100%;
      justify-content: center;
    }

    .lesson-label {
      display: none;
    }
  }
</style>
