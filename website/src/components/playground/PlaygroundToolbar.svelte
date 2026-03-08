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

  let dropdownOpen = $state(false);
  let dropdownRef: HTMLDivElement | undefined = $state();

  function selectExample(key: string) {
    dropdownOpen = false;
    onexample?.(key);
  }

  function handleClickOutside(e: MouseEvent) {
    if (dropdownRef && !dropdownRef.contains(e.target as Node)) {
      dropdownOpen = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') dropdownOpen = false;
  }

  $effect(() => {
    if (dropdownOpen) {
      document.addEventListener('click', handleClickOutside);
      document.addEventListener('keydown', handleKeydown);
      return () => {
        document.removeEventListener('click', handleClickOutside);
        document.removeEventListener('keydown', handleKeydown);
      };
    }
  });
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
      <div class="dropdown" bind:this={dropdownRef}>
        <button
          class="btn btn-secondary dropdown-trigger"
          onclick={() => dropdownOpen = !dropdownOpen}
        >
          Examples
          <svg class="chevron" class:open={dropdownOpen} width="10" height="6" viewBox="0 0 10 6" fill="currentColor">
            <path d="M0 0l5 6 5-6z"/>
          </svg>
        </button>
        {#if dropdownOpen}
          <div class="dropdown-menu">
            {#each Object.entries(EXAMPLES) as [key, example]}
              <button
                class="dropdown-item"
                class:active={selectedExample === key}
                onclick={() => selectExample(key)}
              >
                {example.label}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--color-bg-elevated, #282828);
    border-bottom: 1px solid var(--color-border, #333333);
    flex-shrink: 0;
  }

  .toolbar-left {
    display: flex;
    align-items: center;
  }

  .file-label {
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
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
    transition: background 0.15s ease, color 0.15s ease, border-color 0.15s ease;
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

  .btn-secondary {
    background: var(--color-bg-tertiary, #242424);
    color: var(--color-text-primary, #cccbc7);
    border: 1px solid var(--color-border, #333333);
  }

  .btn-secondary:hover {
    background: var(--color-border-hover, #3d3d3d);
  }

  .btn-outline {
    background: transparent;
    color: var(--color-primary, #d1a847);
    border: 1px solid var(--color-primary, #d1a847);
  }

  .btn-outline:hover {
    background: var(--color-primary-subtle, rgba(209, 168, 71, 0.12));
    color: var(--color-primary-hover, #d9bd7d);
    border-color: var(--color-primary-hover, #d9bd7d);
  }

  .btn-outline:disabled {
    color: var(--color-text-muted, #908f8b);
    border-color: var(--color-text-muted, #908f8b);
    cursor: not-allowed;
  }

  .btn-icon {
    font-size: 0.75rem;
  }

  /* Custom dropdown */
  .dropdown {
    position: relative;
  }

  .dropdown-trigger {
    gap: 0.5rem;
  }

  .chevron {
    transition: transform 0.15s ease;
    opacity: 0.6;
  }

  .chevron.open {
    transform: rotate(180deg);
  }

  .dropdown-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    min-width: 180px;
    background: var(--color-bg-secondary, #161616);
    border: 1px solid var(--color-border, #333333);
    border-radius: 6px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.25);
    padding: 0.25rem;
    z-index: 50;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dropdown-item {
    display: block;
    width: 100%;
    padding: 0.4rem 0.75rem;
    background: transparent;
    border: none;
    border-radius: 4px;
    color: var(--color-text-secondary, #aca9a3);
    font-family: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  .dropdown-item:hover {
    color: var(--color-text-primary, #cccbc7);
    background: rgba(255, 255, 255, 0.06);
  }

  .dropdown-item.active {
    color: var(--color-accent, #6ba591);
    background: var(--color-accent-subtle, rgba(126, 183, 166, 0.15));
  }
</style>
