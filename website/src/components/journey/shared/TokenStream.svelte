<script lang="ts">
  import { onMount } from 'svelte';

  let {
    tokens,
    animated = true,
    onhover = (_index: number | null) => {},
  }: {
    tokens: { type: string; value?: string }[];
    animated?: boolean;
    onhover?: (index: number | null) => void;
  } = $props();

  let visibleCount = $state(0);
  let showAll = $state(false);

  const displayCount = $derived(showAll || !animated ? tokens.length : visibleCount);

  // Token type -> color category
  function tokenColor(type: string): string {
    const t = type.toLowerCase();
    if (['fn', 'let', 'if', 'else', 'for', 'match', 'trait', 'impl', 'type', 'pub', 'use', 'return'].includes(t))
      return 'keyword';
    if (t === 'ident') return 'identifier';
    if (['int', 'float', 'string', 'true', 'false'].includes(t)) return 'literal';
    if (['arrow', 'eq', 'plus', 'minus', 'star', 'slash', 'colon', 'semi', 'comma', 'dot'].includes(t))
      return 'operator';
    if (['lparen', 'rparen', 'lbrace', 'rbrace', 'lbracket', 'rbracket'].includes(t))
      return 'bracket';
    return 'default';
  }

  onMount(() => {
    if (!animated) return;

    let i = 0;
    const interval = setInterval(() => {
      if (i >= tokens.length) {
        clearInterval(interval);
        return;
      }
      visibleCount = ++i;
    }, 35);

    return () => clearInterval(interval);
  });
</script>

<div class="token-stream">
  {#if animated && !showAll && visibleCount < tokens.length}
    <button class="show-all-btn" onclick={() => showAll = true}>
      Show All ({tokens.length - visibleCount} remaining)
    </button>
  {/if}

  <div class="tokens">
    {#each tokens.slice(0, displayCount) as token, i}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="token {tokenColor(token.type)}"
        class:entering={animated && !showAll && i === visibleCount - 1}
        onmouseenter={() => onhover(i)}
        onmouseleave={() => onhover(null)}
      >
        {#if token.value}
          <span class="token-type">{token.type}</span><span class="token-value">({token.value})</span>
        {:else}
          <span class="token-type">{token.type}</span>
        {/if}
      </span>
    {/each}
  </div>
</div>

<style>
  .token-stream {
    position: relative;
  }

  .show-all-btn {
    position: absolute;
    top: var(--space-2, 8px);
    right: var(--space-2, 8px);
    padding: var(--space-1, 4px) var(--space-3, 12px);
    background: var(--color-bg-tertiary, #242424);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    color: var(--color-text-secondary, #aca9a3);
    font-size: 0.6875rem;
    cursor: pointer;
    z-index: 1;
    transition: all 0.15s ease;
  }

  .show-all-btn:hover {
    background: var(--color-bg-elevated, #282828);
    color: var(--color-text-primary, #cccbc7);
  }

  .tokens {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1, 4px);
    padding: var(--space-3, 12px);
  }

  .token {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: var(--radius-sm, 4px);
    font-family: var(--font-mono, monospace);
    font-size: 0.75rem;
    cursor: default;
    transition: transform 0.1s ease, box-shadow 0.1s ease;
  }

  .token:hover {
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .token.entering {
    animation: tokenAppear 150ms ease-out;
  }

  @keyframes tokenAppear {
    from { opacity: 0; transform: scale(0.8); }
    to   { opacity: 1; transform: scale(1); }
  }

  .token-type {
    font-weight: 600;
  }

  .token-value {
    opacity: 0.7;
    margin-left: 1px;
  }

  /* Color categories */
  .token.keyword {
    background: rgba(126, 183, 166, 0.15);
    color: #72c2de;
  }

  .token.identifier {
    background: rgba(220, 220, 220, 0.08);
    color: #dcdcdc;
  }

  .token.literal {
    background: rgba(181, 206, 168, 0.12);
    color: #b5cea8;
  }

  .token.operator {
    background: rgba(212, 212, 212, 0.08);
    color: #d4d4d4;
  }

  .token.bracket {
    background: rgba(255, 215, 0, 0.08);
    color: #ffd700;
  }

  .token.default {
    background: rgba(164, 161, 157, 0.08);
    color: #aca9a3;
  }
</style>
