<script lang="ts">
  import type { AstNode } from '../journey-types';

  let {
    nodes,
    onselect = (_label: string) => {},
  }: {
    nodes: AstNode[];
    onselect?: (label: string) => void;
  } = $props();

  let expandedNodes: Set<string> = $state(new Set());

  // Auto-expand first two levels
  function initExpanded(nodes: AstNode[], path: string = '', depth: number = 0): string[] {
    const keys: string[] = [];
    for (let i = 0; i < nodes.length; i++) {
      const key = `${path}/${i}`;
      if (depth < 2) {
        keys.push(key);
        keys.push(...initExpanded(nodes[i].children, key, depth + 1));
      }
    }
    return keys;
  }

  // Initialize expanded state
  $effect(() => {
    expandedNodes = new Set(initExpanded(nodes));
  });

  function toggleNode(key: string) {
    const next = new Set(expandedNodes);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    expandedNodes = next;
  }

  // Depth-based colors
  const depthColors = [
    '#569cd6', // blue
    '#6ba591', // teal
    '#dcdcaa', // yellow
    '#c586c0', // purple
    '#ce9178', // orange
    '#9cdcfe', // light blue
  ];

  function depthColor(depth: number): string {
    return depthColors[depth % depthColors.length];
  }
</script>

<div class="ast-tree">
  {#each nodes as node, i}
    {@const key = `/${i}`}
    {@render treeNode(node, key, 0)}
  {/each}
</div>

{#snippet treeNode(node: AstNode, key: string, depth: number)}
  {@const isExpanded = expandedNodes.has(key)}
  {@const hasChildren = node.children.length > 0}

  <div class="tree-row" style="padding-left: {depth * 20}px">
    <button
      class="tree-node"
      class:expandable={hasChildren}
      onclick={() => {
        if (hasChildren) toggleNode(key);
        onselect(node.label);
      }}
    >
      {#if hasChildren}
        <span class="expand-icon" class:expanded={isExpanded}>
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path d="M3 2L7 5L3 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
        </span>
      {:else}
        <span class="leaf-icon">&#x2022;</span>
      {/if}
      <span class="node-label" style="color: {depthColor(depth)}">{node.label}</span>
      {#if hasChildren}
        <span class="child-count">{node.children.length}</span>
      {/if}
    </button>
  </div>

  {#if hasChildren && isExpanded}
    <div class="tree-children" style="animation: treeExpand 200ms ease-out">
      {#each node.children as child, ci}
        {@render treeNode(child, `${key}/${ci}`, depth + 1)}
      {/each}
    </div>
  {/if}
{/snippet}

<style>
  .ast-tree {
    padding: var(--space-3, 12px);
    font-family: var(--font-mono, monospace);
    font-size: 0.8125rem;
  }

  .tree-row {
    display: flex;
    align-items: center;
  }

  .tree-node {
    display: flex;
    align-items: center;
    gap: var(--space-1, 4px);
    padding: 2px var(--space-2, 8px);
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-sm, 4px);
    width: 100%;
    text-align: left;
    transition: background 0.1s ease;
  }

  .tree-node:hover {
    background: rgba(126, 183, 166, 0.08);
  }

  .tree-node.expandable {
    cursor: pointer;
  }

  .expand-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    color: var(--color-text-muted, #908f8b);
    transition: transform 0.15s ease;
    flex-shrink: 0;
  }

  .expand-icon.expanded {
    transform: rotate(90deg);
  }

  .leaf-icon {
    width: 14px;
    text-align: center;
    color: var(--color-text-muted, #908f8b);
    opacity: 0.4;
    flex-shrink: 0;
  }

  .node-label {
    font-weight: 500;
    white-space: nowrap;
  }

  .child-count {
    font-size: 0.625rem;
    color: var(--color-text-muted, #908f8b);
    background: var(--color-bg-tertiary, #242424);
    padding: 0 6px;
    border-radius: var(--radius-full, 9999px);
    margin-left: var(--space-1, 4px);
  }

  .tree-children {
    overflow: hidden;
  }

  @keyframes treeExpand {
    from { opacity: 0; max-height: 0; }
    to   { opacity: 1; max-height: 1000px; }
  }
</style>
