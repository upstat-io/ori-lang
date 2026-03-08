<script lang="ts">
  import { onMount } from 'svelte';
  import type { JourneyData, PhaseId } from './journey-types';
  import { PHASES } from './journey-types';
  import JourneyProgressBar from './JourneyProgressBar.svelte';
  import SourcePhase from './phases/SourcePhase.svelte';
  import LexerPhase from './phases/LexerPhase.svelte';
  import ParserPhase from './phases/ParserPhase.svelte';
  import TypeCheckerPhase from './phases/TypeCheckerPhase.svelte';
  import ArcPhase from './phases/ArcPhase.svelte';
  import EvalPhase from './phases/EvalPhase.svelte';
  import LlvmPhase from './phases/LlvmPhase.svelte';
  import ScorePhase from './phases/ScorePhase.svelte';

  let { data }: { data: JourneyData } = $props();

  let currentPhase: PhaseId = $state('source');
  let visitedPhases: Set<PhaseId> = $state(new Set(['source']));
  let direction: 'forward' | 'backward' = $state('forward');
  let reducedMotion = $state(false);

  const currentIndex = $derived(PHASES.findIndex(p => p.id === currentPhase));

  function navigateTo(phase: PhaseId) {
    const newIndex = PHASES.findIndex(p => p.id === phase);
    if (newIndex === currentIndex) return;

    direction = newIndex > currentIndex ? 'forward' : 'backward';
    currentPhase = phase;
    visitedPhases = new Set([...visitedPhases, phase]);
    updateHash(phase);
  }

  function navigateNext() {
    if (currentIndex < PHASES.length - 1) {
      navigateTo(PHASES[currentIndex + 1].id);
    }
  }

  function navigatePrev() {
    if (currentIndex > 0) {
      navigateTo(PHASES[currentIndex - 1].id);
    }
  }

  function updateHash(phase: PhaseId) {
    const url = new URL(window.location.href);
    url.hash = `phase=${phase}`;
    history.replaceState(null, '', url.toString());
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

    if (e.key === 'ArrowRight' || e.key === 'l') {
      e.preventDefault();
      navigateNext();
    } else if (e.key === 'ArrowLeft' || e.key === 'h') {
      e.preventDefault();
      navigatePrev();
    } else if (e.key >= '1' && e.key <= '8') {
      e.preventDefault();
      const idx = parseInt(e.key, 10) - 1;
      if (idx < PHASES.length) navigateTo(PHASES[idx].id);
    }
  }

  // Touch swipe support
  let touchStartX = 0;
  function handleTouchStart(e: TouchEvent) {
    touchStartX = e.touches[0].clientX;
  }

  function handleTouchEnd(e: TouchEvent) {
    const diff = e.changedTouches[0].clientX - touchStartX;
    if (Math.abs(diff) > 50) {
      if (diff < 0) navigateNext();
      else navigatePrev();
    }
  }

  onMount(() => {
    reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    // Read initial phase from hash
    const hash = window.location.hash;
    const phaseMatch = hash.match(/phase=(\w+)/);
    if (phaseMatch) {
      const phase = phaseMatch[1] as PhaseId;
      if (PHASES.some(p => p.id === phase)) {
        currentPhase = phase;
        visitedPhases = new Set([...visitedPhases, phase]);
      }
    }

    window.addEventListener('keydown', handleKeydown);
    return () => {
      window.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="journey-shell"
  ontouchstart={handleTouchStart}
  ontouchend={handleTouchEnd}
>
  <div class="journey-layout">
    <div class="phase-panel">
      <JourneyProgressBar
        phases={PHASES}
        currentPhase={currentPhase}
        {visitedPhases}
        onnavigate={navigateTo}
      />

      <div class="phase-content" class:reduced-motion={reducedMotion}>
        {#key currentPhase}
          <div class="phase-slide" class:slide-forward={direction === 'forward'} class:slide-backward={direction === 'backward'}>
            {#if currentPhase === 'source'}
              <SourcePhase {data} />
            {:else if currentPhase === 'lexer'}
              <LexerPhase lexer={data.lexer} />
            {:else if currentPhase === 'parser'}
              <ParserPhase parser={data.parser} />
            {:else if currentPhase === 'typechecker'}
              <TypeCheckerPhase typeChecker={data.typeChecker} />
            {:else if currentPhase === 'arc'}
              <ArcPhase arc={data.arc} />
            {:else if currentPhase === 'eval'}
              <EvalPhase interpreter={data.interpreter} />
            {:else if currentPhase === 'llvm'}
              <LlvmPhase llvm={data.llvm} />
            {:else if currentPhase === 'score'}
              <ScorePhase
                score={data.score}
                findings={data.findings}
                scrutiny={data.scrutiny}
                verdict={data.verdict}
              />
            {/if}
          </div>
        {/key}
      </div>

      <div class="phase-nav-footer">
        <button class="nav-btn" onclick={navigatePrev} disabled={currentIndex === 0}>
          <span class="nav-arrow">&larr;</span> Prev
        </button>
        <span class="phase-label">
          {currentIndex + 1} / {PHASES.length}: {PHASES[currentIndex]?.name}
        </span>
        <button class="nav-btn" onclick={navigateNext} disabled={currentIndex === PHASES.length - 1}>
          Next <span class="nav-arrow">&rarr;</span>
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .journey-shell {
    position: fixed;
    top: var(--header-height, 56px);
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-primary, #0b0b0b);
    overflow: hidden;
    z-index: 1;
  }

  .journey-layout {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .phase-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .phase-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    position: relative;
  }

  .phase-slide {
    animation: slideIn 250ms ease-out;
  }

  .reduced-motion .phase-slide {
    animation: none;
  }

  .phase-slide.slide-forward {
    animation-name: slideInRight;
  }

  .phase-slide.slide-backward {
    animation-name: slideInLeft;
  }

  @keyframes slideInRight {
    from { opacity: 0; transform: translateX(30px); }
    to   { opacity: 1; transform: translateX(0); }
  }

  @keyframes slideInLeft {
    from { opacity: 0; transform: translateX(-30px); }
    to   { opacity: 1; transform: translateX(0); }
  }

  .phase-nav-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space-2, 8px) var(--space-4, 16px);
    border-top: 1px solid var(--color-border, #333333);
    background: var(--color-bg-secondary, #161616);
    flex-shrink: 0;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: var(--space-1, 4px);
    padding: var(--space-2, 8px) var(--space-3, 12px);
    background: var(--color-bg-tertiary, #242424);
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    color: var(--color-text-primary, #cccbc7);
    font-size: 0.8125rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .nav-btn:hover:not(:disabled) {
    background: var(--color-bg-elevated, #282828);
    border-color: var(--color-accent, #7eb7a6);
  }

  .nav-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .nav-arrow {
    font-size: 0.875rem;
  }

  .phase-label {
    font-size: 0.75rem;
    color: var(--color-text-muted, #908f8b);
    font-family: var(--font-mono, monospace);
  }

  @media (max-width: 768px) {
    .phase-nav-footer {
      padding: var(--space-2, 8px);
    }

    .nav-btn {
      padding: var(--space-1, 4px) var(--space-2, 8px);
      font-size: 0.75rem;
    }
  }
</style>
