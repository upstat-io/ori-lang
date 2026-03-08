<script lang="ts">
  import { onMount } from 'svelte';
  import MonacoEditor from '../playground/MonacoEditor.svelte';
  import { initWasm, runOri, formatOri, isReady } from '../playground/wasm-runner';
  import type { RunResult } from '../playground/types';
  import type { TutorialManifest, TutorialLesson } from './tutorial-types';
  import TutorialSidebar from './TutorialSidebar.svelte';
  import TutorialInstructions from './TutorialInstructions.svelte';
  import TutorialToolbar from './TutorialToolbar.svelte';
  import TutorialOutput from './TutorialOutput.svelte';
  import {
    loadProgress,
    markCompleted,
    saveCode,
    getSavedCode,
    savePosition,
  } from './tutorial-progress';

  let { initialSlug = '' }: { initialSlug?: string } = $props();

  // Core state
  let manifest: TutorialManifest | null = $state(null);
  let currentSlug = $state('');
  let code = $state('');

  // Run state (same model as Playground)
  let result: RunResult | null = $state(null);
  let elapsed = $state('');
  let status: 'idle' | 'running' | 'success' | 'error' = $state('idle');

  // Tutorial-specific state
  let validated: boolean | null = $state(null);
  let completedSlugs: string[] = $state([]);
  let sidebarCollapsed = $state(false);
  let wasmReady = $state(false);
  let loading = $state(true);

  // Resizable panes
  let instructionsWidth = $state(40); // percentage
  let resizing = $state(false);

  // Derived: current lesson data
  const currentLesson: TutorialLesson | null = $derived(
    manifest && currentSlug ? manifest.lessons[currentSlug] ?? null : null
  );

  // Derived: flat ordered list of all lesson slugs for prev/next
  const allSlugs: string[] = $derived(
    manifest ? manifest.chapters.flatMap(ch => ch.lessons.map(l => l.slug)) : []
  );

  const currentIndex = $derived(allSlugs.indexOf(currentSlug));
  const hasPrev = $derived(currentIndex > 0);
  const hasNext = $derived(currentIndex >= 0 && currentIndex < allSlugs.length - 1);

  // Navigate to a lesson by slug
  function navigateTo(slug: string) {
    if (!manifest || !manifest.lessons[slug]) return;
    if (slug === currentSlug) return;

    // Save current code before leaving
    if (currentSlug && code) {
      saveCode(currentSlug, code);
    }

    currentSlug = slug;
    const lesson = manifest.lessons[slug];

    // Restore saved code or use starter
    code = getSavedCode(slug) ?? lesson.starterCode;

    // Reset run state
    result = null;
    elapsed = '';
    status = 'idle';
    validated = null;

    // Update URL and save position
    const url = `/tutorial/${slug}`;
    if (window.location.pathname !== url) {
      history.pushState({ slug }, '', url);
    }
    savePosition(slug);
  }

  // Validation logic: compare real output against expected
  function validateOutput(runResult: RunResult, lesson: TutorialLesson): boolean {
    const { validation } = lesson;
    if (validation.outputMatch === 'none') return true;
    if (!runResult.success) return false;

    let actual = '';
    if (runResult.printed) actual += runResult.printed;
    if (runResult.output) {
      if (actual) actual += '\n';
      actual += runResult.output;
    }
    actual = actual.trim();
    const expected = validation.expectedOutput.trim();

    switch (validation.outputMatch) {
      case 'exact': return actual === expected;
      case 'contains': return actual.includes(expected);
      case 'regex': return new RegExp(expected).test(actual);
      default: return true;
    }
  }

  // Handlers
  function handleFormat(): boolean {
    if (!isReady()) return false;
    const formatResult = formatOri(code, 100);
    if (formatResult.success && formatResult.formatted) {
      code = formatResult.formatted;
      return true;
    } else if (formatResult.error) {
      result = { success: false, error: formatResult.error, error_type: 'parse' };
      status = 'error';
      validated = null;
      return false;
    }
    return true;
  }

  async function handleRun() {
    if (!isReady() || !currentLesson) return;

    if (!handleFormat()) return;

    status = 'running';
    result = null;
    elapsed = '';
    validated = null;

    // Let UI update
    await new Promise(r => setTimeout(r, 10));

    try {
      const res = runOri(code);
      result = res.result;
      elapsed = res.elapsed;
      status = result.success ? 'success' : 'error';

      // Validate against lesson expectations
      if (currentLesson) {
        const passed = validateOutput(result, currentLesson);
        validated = passed;
        if (passed && !completedSlugs.includes(currentSlug)) {
          completedSlugs = [...completedSlugs, currentSlug];
          markCompleted(currentSlug);
        }
      }
    } catch (e: any) {
      result = { success: false, error: `Internal error: ${e.message}`, error_type: 'runtime' };
      status = 'error';
      validated = null;
    }

    // Save code after running
    if (currentSlug) {
      saveCode(currentSlug, code);
    }
  }

  function handleReset() {
    if (!currentLesson) return;
    code = currentLesson.starterCode;
    result = null;
    elapsed = '';
    status = 'idle';
    validated = null;
  }

  function handleSolution() {
    if (!currentLesson) return;
    code = currentLesson.solutionCode;
    result = null;
    elapsed = '';
    status = 'idle';
    validated = null;
  }

  function handlePrev() {
    if (hasPrev) navigateTo(allSlugs[currentIndex - 1]);
  }

  function handleNext() {
    if (hasNext) navigateTo(allSlugs[currentIndex + 1]);
  }

  // Resizable divider
  function startResize(e: MouseEvent) {
    e.preventDefault();
    resizing = true;

    const onMove = (ev: MouseEvent) => {
      const container = (e.target as HTMLElement).parentElement;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const pct = ((ev.clientX - rect.left) / rect.width) * 100;
      instructionsWidth = Math.max(20, Math.min(60, pct));
    };

    const onUp = () => {
      resizing = false;
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  // Keyboard shortcuts
  function handleKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      handleRun();
    }
  }

  // Init
  onMount(async () => {
    // Fetch manifest
    try {
      const resp = await fetch('/tutorial-manifest.json');
      manifest = await resp.json();
    } catch (e) {
      console.error('Failed to load tutorial manifest:', e);
      loading = false;
      return;
    }

    // Load saved progress
    const progress = loadProgress();
    completedSlugs = progress.completed;

    // Navigate to initial slug
    navigateTo(initialSlug || allSlugs[0] || '');

    // Init WASM
    wasmReady = await initWasm();
    loading = false;

    // Handle browser back/forward
    window.addEventListener('popstate', (e) => {
      const slug = e.state?.slug || extractSlugFromPath();
      if (slug && manifest?.lessons[slug]) {
        navigateTo(slug);
      }
    });

    // Keyboard shortcuts
    window.addEventListener('keydown', handleKeydown);

    return () => {
      window.removeEventListener('keydown', handleKeydown);
    };
  });

  function extractSlugFromPath(): string {
    const match = window.location.pathname.match(/^\/tutorial\/(.+)/);
    return match ? match[1] : '';
  }
</script>

<div class="tutorial-shell">
  {#if loading}
    <div class="loading">Loading tutorial...</div>
  {:else if !manifest}
    <div class="loading">Failed to load tutorial content.</div>
  {:else}
    <div class="tutorial-layout">
      <TutorialSidebar
        chapters={manifest.chapters}
        {currentSlug}
        {completedSlugs}
        collapsed={sidebarCollapsed}
        onnavigate={navigateTo}
        ontoggle={() => sidebarCollapsed = !sidebarCollapsed}
      />

      <div class="content-area">
        <div class="panels" class:resizing>
          <div class="instructions-panel" style="flex: 0 0 {instructionsWidth}%;">
            {#if currentLesson}
              {#key currentSlug}
                <TutorialInstructions
                  contentHtml={currentLesson.contentHtml}
                  title={currentLesson.title}
                />
              {/key}
            {/if}
          </div>

          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="resize-handle" onmousedown={startResize}></div>

          <div class="editor-area">
            <TutorialToolbar
              running={status === 'running'}
              {hasNext}
              {hasPrev}
              lessonTitle={currentLesson?.title ?? ''}
              onrun={handleRun}
              onformat={() => handleFormat()}
              onreset={handleReset}
              onsolution={handleSolution}
              onnext={handleNext}
              onprev={handlePrev}
            />
            <div class="editor-panel">
              <MonacoEditor bind:value={code} fontSize={14} onrun={handleRun} />
            </div>
            <TutorialOutput
              {result}
              {elapsed}
              {status}
              {validated}
              hints={currentLesson?.validation.hints ?? []}
              expectedOutput={currentLesson?.validation.expectedOutput ?? ''}
              outputMatch={currentLesson?.validation.outputMatch ?? 'none'}
            />
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .tutorial-shell {
    /* Position fixed below the header — bypasses all parent flex/height
       propagation issues. The header is sticky at 56px (--header-height). */
    position: fixed;
    top: var(--header-height, 56px);
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-secondary, #161616);
    overflow: hidden;
    z-index: 1;
  }

  .loading {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-muted, #908f8b);
    font-size: 0.875rem;
  }

  .tutorial-layout {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .content-area {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .panels {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .panels.resizing {
    user-select: none;
    cursor: col-resize;
  }

  .instructions-panel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
    border-right: 1px solid var(--color-border, #333333);
  }

  .resize-handle {
    width: 4px;
    cursor: col-resize;
    background: transparent;
    flex-shrink: 0;
    transition: background 0.15s ease;
    position: relative;
  }

  .resize-handle:hover,
  .panels.resizing .resize-handle {
    background: var(--color-primary, #d1a847);
  }

  .editor-area {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .editor-panel {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  /* Mobile: stack vertically */
  @media (max-width: 768px) {
    .panels {
      flex-direction: column;
    }

    .instructions-panel {
      flex: 0 0 auto !important;
      max-height: 40vh;
      border-right: none;
      border-bottom: 1px solid var(--color-border, #333333);
    }

    .resize-handle {
      display: none;
    }

    .editor-area {
      flex: 1;
    }
  }
</style>
