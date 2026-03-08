<script lang="ts">
  import type { TutorialChapter } from './tutorial-types';

  let {
    chapters = [],
    currentSlug = '',
    completedSlugs = [],
    collapsed = false,
    onnavigate,
    ontoggle,
  }: {
    chapters: TutorialChapter[];
    currentSlug: string;
    completedSlugs: string[];
    collapsed?: boolean;
    onnavigate?: (slug: string) => void;
    ontoggle?: () => void;
  } = $props();
</script>

{#if !collapsed}
  <aside class="sidebar">
    <div class="sidebar-header">
      <span class="sidebar-title">Tutorial</span>
      <button class="collapse-btn" onclick={ontoggle} aria-label="Collapse sidebar">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M10 4L6 8L10 12" />
        </svg>
      </button>
    </div>
    <nav class="sidebar-nav">
      {#each chapters as chapter}
        <div class="chapter">
          <div class="chapter-title">{chapter.title}</div>
          <ul class="lesson-list">
            {#each chapter.lessons as lesson}
              <li>
                <button
                  class="lesson-link"
                  class:active={lesson.slug === currentSlug}
                  class:completed={completedSlugs.includes(lesson.slug)}
                  onclick={() => onnavigate?.(lesson.slug)}
                >
                  <span class="lesson-status">
                    {#if completedSlugs.includes(lesson.slug)}
                      <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                        <path d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"/>
                      </svg>
                    {:else}
                      <span class="lesson-dot"></span>
                    {/if}
                  </span>
                  <span class="lesson-title">{lesson.title}</span>
                </button>
              </li>
            {/each}
          </ul>
        </div>
      {/each}
    </nav>
  </aside>
{:else}
  <button class="expand-btn" onclick={ontoggle} aria-label="Expand sidebar">
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
      <path d="M6 4L10 8L6 12" />
    </svg>
  </button>
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 220px;
    min-width: 220px;
    background: var(--color-bg-secondary, #161616);
    border-right: 1px solid var(--color-border, #333333);
    overflow: hidden;
  }

  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: var(--color-bg-elevated, #282828);
    border-bottom: 1px solid var(--color-border, #333333);
    flex-shrink: 0;
  }

  .sidebar-title {
    font-size: 0.8125rem;
    font-weight: 600;
    color: var(--color-text-primary, #cccbc7);
  }

  .collapse-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--color-text-muted, #908f8b);
    cursor: pointer;
    padding: 0.25rem;
    border-radius: 3px;
    transition: all 0.15s ease;
  }

  .collapse-btn:hover {
    color: var(--color-text-primary, #cccbc7);
    background: var(--color-bg-tertiary, #242424);
  }

  .sidebar-nav {
    flex: 1;
    overflow-y: auto;
    padding: 0.5rem 0;
  }

  .chapter {
    margin-bottom: 0.5rem;
  }

  .chapter-title {
    padding: 0.375rem 1rem;
    font-size: 0.6875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-text-muted, #908f8b);
  }

  .lesson-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .lesson-link {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.375rem 1rem;
    background: none;
    border: none;
    font-family: inherit;
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #aca9a3);
    cursor: pointer;
    text-align: left;
    transition: all 0.15s ease;
  }

  .lesson-link:hover {
    color: var(--color-text-primary, #cccbc7);
    background: var(--color-bg-tertiary, #242424);
  }

  .lesson-link.active {
    color: var(--color-primary, #d1a847);
    background: rgba(126, 183, 166, 0.1);
  }

  .lesson-link.completed .lesson-status {
    color: var(--color-success, #6ba591);
  }

  .lesson-status {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .lesson-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-text-muted, #908f8b);
  }

  .lesson-link.active .lesson-dot {
    background: var(--color-primary, #d1a847);
  }

  .lesson-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Expand button when collapsed */
  .expand-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    background: var(--color-bg-secondary, #161616);
    border: none;
    border-right: 1px solid var(--color-border, #333333);
    color: var(--color-text-muted, #908f8b);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .expand-btn:hover {
    color: var(--color-text-primary, #cccbc7);
    background: var(--color-bg-tertiary, #242424);
  }

  /* Scrollbar */
  .sidebar-nav::-webkit-scrollbar {
    width: 4px;
  }
  .sidebar-nav::-webkit-scrollbar-track {
    background: transparent;
  }
  .sidebar-nav::-webkit-scrollbar-thumb {
    background: var(--color-text-muted, #908f8b);
    border-radius: 2px;
  }
</style>
