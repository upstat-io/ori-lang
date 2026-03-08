<script lang="ts">
  let {
    contentHtml = '',
    title = '',
  }: {
    contentHtml: string;
    title?: string;
  } = $props();
</script>

<div class="instructions">
  <div class="instructions-content">
    {@html contentHtml}
  </div>
</div>

<style>
  .instructions {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .instructions-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    font-size: 0.9375rem;
    line-height: 1.7;
    color: var(--color-text-primary, #cccbc7);
  }

  /* Prose styling */
  .instructions-content :global(h1) {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 1rem;
    color: var(--color-text-primary, #cccbc7);
  }

  .instructions-content :global(h2) {
    font-size: 1.125rem;
    font-weight: 600;
    margin: 1.5rem 0 0.75rem;
    color: var(--color-text-primary, #cccbc7);
  }

  .instructions-content :global(p) {
    margin: 0 0 0.75rem;
    color: var(--color-text-secondary, #aca9a3);
  }

  .instructions-content :global(code) {
    background: var(--color-bg-tertiary, #242424);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-family: var(--font-mono, 'JetBrains Mono', monospace);
    font-size: 0.85em;
    color: var(--color-primary, #d1a847);
  }

  .instructions-content :global(pre) {
    background: var(--color-bg-primary, #0b0b0b);
    border: 1px solid var(--color-border, #333333);
    border-radius: 6px;
    padding: 1rem;
    margin: 0.75rem 0;
    overflow-x: auto;
  }

  .instructions-content :global(pre code) {
    background: none;
    padding: 0;
    font-size: 0.8125rem;
    color: var(--color-text-primary, #cccbc7);
  }

  .instructions-content :global(strong) {
    color: var(--color-text-primary, #cccbc7);
    font-weight: 600;
  }

  .instructions-content :global(ul),
  .instructions-content :global(ol) {
    margin: 0.5rem 0 0.75rem;
    padding-left: 1.5rem;
    color: var(--color-text-secondary, #aca9a3);
  }

  .instructions-content :global(li) {
    margin-bottom: 0.375rem;
  }

  .instructions-content :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 0.75rem 0;
    font-size: 0.875rem;
  }

  .instructions-content :global(th),
  .instructions-content :global(td) {
    padding: 0.5rem 0.75rem;
    border: 1px solid var(--color-border, #333333);
    text-align: left;
  }

  .instructions-content :global(th) {
    background: var(--color-bg-tertiary, #242424);
    font-weight: 600;
    color: var(--color-text-primary, #cccbc7);
  }

  .instructions-content :global(td) {
    color: var(--color-text-secondary, #aca9a3);
  }

  /* Scrollbar */
  .instructions-content::-webkit-scrollbar {
    width: 6px;
  }
  .instructions-content::-webkit-scrollbar-track {
    background: transparent;
  }
  .instructions-content::-webkit-scrollbar-thumb {
    background: var(--color-text-muted, #908f8b);
    border-radius: 3px;
  }
</style>
