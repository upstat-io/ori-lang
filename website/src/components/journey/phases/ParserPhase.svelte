<script lang="ts">
  import type { ParserData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';
  import AstTreeView from '../shared/AstTreeView.svelte';
  import CodeBlock from '../shared/CodeBlock.svelte';

  let {
    parser,
    onhighlight = (_line: number | null) => {},
  }: {
    parser: ParserData;
    onhighlight?: (line: number | null) => void;
  } = $props();

  let viewMode: 'tree' | 'text' = $state('tree');
</script>

<div class="parser-phase">
  <h2>Parser</h2>

  <PhaseIntro intro={parser.intro} metrics={parser.metrics} />

  <div class="view-toggle">
    <button class:active={viewMode === 'tree'} onclick={() => viewMode = 'tree'}>
      Interactive Tree
    </button>
    <button class:active={viewMode === 'text'} onclick={() => viewMode = 'text'}>
      Text View
    </button>
  </div>

  <div class="ast-section">
    {#if viewMode === 'tree' && parser.nodes.length > 0}
      <div class="tree-container">
        <AstTreeView nodes={parser.nodes} />
      </div>
    {:else}
      <CodeBlock code={parser.astText} language="text" showLineNumbers={false} />
    {/if}
  </div>
</div>

<style>
  .parser-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  .view-toggle {
    display: flex;
    gap: 2px;
    margin-bottom: var(--space-4, 16px);
    background: var(--color-bg-tertiary, #242424);
    border-radius: var(--radius-md, 8px);
    padding: 2px;
    width: fit-content;
  }

  .view-toggle button {
    padding: var(--space-1, 4px) var(--space-3, 12px);
    background: none;
    border: none;
    border-radius: var(--radius-sm, 4px);
    color: var(--color-text-muted, #908f8b);
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .view-toggle button.active {
    background: var(--color-bg-secondary, #161616);
    color: var(--color-text-primary, #cccbc7);
  }

  .view-toggle button:hover:not(.active) {
    color: var(--color-text-secondary, #aca9a3);
  }

  .tree-container {
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    background: var(--color-bg-code, #111111);
    max-height: 600px;
    overflow: auto;
  }
</style>
