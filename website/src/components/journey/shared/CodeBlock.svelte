<script lang="ts">
  let {
    code,
    language = 'text',
    showLineNumbers = true,
    maxHeight = '',
    highlightLines = [] as number[],
  }: {
    code: string;
    language?: string;
    showLineNumbers?: boolean;
    maxHeight?: string;
    highlightLines?: number[];
  } = $props();

  const lines = $derived(code.split('\n'));
</script>

<div class="code-block" style={maxHeight ? `max-height: ${maxHeight}` : ''}>
  <div class="code-header">
    <span class="code-lang">{language}</span>
  </div>
  <pre class="code-pre"><code>{#each lines as line, i}<span
      class="code-line"
      class:highlighted={highlightLines.includes(i + 1)}
    >{#if showLineNumbers}<span class="ln">{String(i + 1).padStart(3, ' ')}</span>{/if}{line}
</span>{/each}</code></pre>
</div>

<style>
  .code-block {
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    overflow: hidden;
    background: var(--color-bg-code, #111111);
  }

  .code-header {
    display: flex;
    align-items: center;
    padding: var(--space-1, 4px) var(--space-3, 12px);
    background: var(--color-bg-tertiary, #242424);
    border-bottom: 1px solid var(--color-border, #333333);
  }

  .code-lang {
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #908f8b);
  }

  .code-pre {
    margin: 0;
    padding: var(--space-3, 12px) 0;
    overflow: auto;
    font-family: var(--font-mono, 'JetBrains Mono', 'Fira Code', monospace);
    font-size: 0.8125rem;
    line-height: 1.6;
  }

  code {
    display: block;
  }

  .code-line {
    display: block;
    padding: 0 var(--space-3, 12px);
  }

  .code-line.highlighted {
    background: rgba(126, 183, 166, 0.1);
  }

  .ln {
    display: inline-block;
    width: 3em;
    text-align: right;
    margin-right: 1em;
    color: var(--color-text-muted, #908f8b);
    opacity: 0.4;
    user-select: none;
  }
</style>
