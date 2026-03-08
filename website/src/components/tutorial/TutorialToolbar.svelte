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
    background: var(--color-bg-elevated, #282828);
    border-bottom: 1px solid var(--color-border, #333333);
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
    color: var(--color-text-secondary, #aca9a3);
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
    background: var(--color-primary, #d1a847);
    color: var(--color-primary-text, #181010);
  }

  .btn-primary:hover {
    background: var(--color-primary-hover, #d9bd7d);
  }

  .btn-primary:disabled {
    background: var(--color-text-muted, #908f8b);
    cursor: not-allowed;
  }

  .btn-outline {
    background: transparent;
    color: var(--color-primary-hover, #d9bd7d);
    border: 1px solid var(--color-primary-hover, #d9bd7d);
  }

  .btn-outline:hover {
    background: var(--color-primary-subtle, rgba(209, 168, 71, 0.12));
    color: var(--color-primary, #d1a847);
    border-color: var(--color-primary, #d1a847);
  }

  .btn-outline:disabled {
    color: var(--color-text-muted, #908f8b);
    border-color: var(--color-text-muted, #908f8b);
    cursor: not-allowed;
  }

  .btn-ghost {
    background: transparent;
    color: var(--color-text-secondary, #aca9a3);
  }

  .btn-ghost:hover {
    color: var(--color-text-primary, #cccbc7);
    background: var(--color-bg-tertiary, #242424);
  }

  .btn-nav {
    background: transparent;
    color: var(--color-text-secondary, #aca9a3);
    padding: 0.375rem 0.5rem;
  }

  .btn-nav:hover:not(:disabled) {
    color: var(--color-text-primary, #cccbc7);
    background: var(--color-bg-tertiary, #242424);
  }

  .btn-nav:disabled {
    color: var(--color-text-muted, #908f8b);
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
