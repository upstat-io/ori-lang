<script lang="ts">
  import type { PhaseId } from './journey-types';

  let {
    phases,
    currentPhase,
    visitedPhases,
    onnavigate,
  }: {
    phases: { id: PhaseId; name: string; index: number }[];
    currentPhase: PhaseId;
    visitedPhases: Set<PhaseId>;
    onnavigate: (phase: PhaseId) => void;
  } = $props();
</script>

<nav class="progress-bar" aria-label="Pipeline phases">
  <div class="progress-track">
    {#each phases as phase, i}
      {@const isCurrent = phase.id === currentPhase}
      {@const isVisited = visitedPhases.has(phase.id) && !isCurrent}

      {#if i > 0}
        <div class="connector" class:active={isVisited || isCurrent}></div>
      {/if}

      <button
        class="phase-step"
        class:current={isCurrent}
        class:visited={isVisited}
        onclick={() => onnavigate(phase.id)}
        aria-current={isCurrent ? 'step' : undefined}
        title={phase.name}
      >
        <span class="step-circle">
          {#if isVisited}
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
              <path d="M2 5L4 7L8 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {:else}
            {i + 1}
          {/if}
        </span>
        <span class="step-label">{phase.name}</span>
      </button>
    {/each}
  </div>
</nav>

<style>
  .progress-bar {
    flex-shrink: 0;
    padding: var(--space-3, 12px) var(--space-4, 16px);
    background: var(--color-bg-secondary, #161616);
    border-bottom: 1px solid var(--color-border, #333333);
    overflow-x: auto;
    scrollbar-width: thin;
  }

  .progress-track {
    display: flex;
    align-items: center;
    gap: 0;
    min-width: max-content;
  }

  .connector {
    flex: 0 0 auto;
    width: 24px;
    height: 2px;
    background: var(--color-bg-tertiary, #242424);
    transition: background 0.2s ease;
  }

  .connector.active {
    background: var(--color-accent, #7eb7a6);
  }

  .phase-step {
    display: flex;
    align-items: center;
    gap: var(--space-2, 8px);
    padding: var(--space-1, 4px) var(--space-2, 8px);
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-md, 8px);
    transition: all 0.15s ease;
    flex-shrink: 0;
  }

  .phase-step:hover {
    background: var(--color-bg-tertiary, #242424);
  }

  .step-circle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    font-size: 0.6875rem;
    font-weight: 600;
    font-family: var(--font-mono, monospace);
    background: var(--color-bg-tertiary, #242424);
    color: var(--color-text-muted, #908f8b);
    border: 2px solid transparent;
    transition: all 0.2s ease;
    flex-shrink: 0;
  }

  .phase-step.current .step-circle {
    background: rgba(126, 183, 166, 0.15);
    color: var(--color-accent, #7eb7a6);
    border-color: var(--color-accent, #7eb7a6);
  }

  .phase-step.visited .step-circle {
    background: rgba(126, 183, 166, 0.15);
    color: var(--color-success, #7eb7a6);
  }

  .step-label {
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--color-text-muted, #908f8b);
    white-space: nowrap;
  }

  .phase-step.current .step-label {
    color: var(--color-accent, #7eb7a6);
    font-weight: 600;
  }

  .phase-step.visited .step-label {
    color: var(--color-text-secondary, #aca9a3);
  }

  /* Mobile: hide labels, scrollable */
  @media (max-width: 768px) {
    .progress-bar {
      padding: var(--space-2, 8px) var(--space-3, 12px);
    }

    .step-label {
      display: none;
    }

    .connector {
      width: 16px;
    }

    .phase-step.current .step-label {
      display: inline;
    }
  }
</style>
