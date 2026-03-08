<script lang="ts">
  import type { TypeCheckerData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';
  import CodeBlock from '../shared/CodeBlock.svelte';

  let { typeChecker }: { typeChecker: TypeCheckerData } = $props();
</script>

<div class="typechecker-phase">
  <h2>Type Checker</h2>

  <PhaseIntro intro={typeChecker.intro} metrics={typeChecker.metrics} />

  {#if typeChecker.annotatedSource}
    <div class="annotated-section">
      <h3>Inferred Types</h3>
      <CodeBlock code={typeChecker.annotatedSource} language="ori" />
    </div>
  {/if}
</div>

<style>
  .typechecker-phase {
    padding: var(--space-6, 24px);
  }

  h2 {
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-4, 16px) 0;
  }

  h3 {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--color-text-primary, #cccbc7);
    margin: 0 0 var(--space-3, 12px) 0;
  }

  .annotated-section {
    margin-top: var(--space-4, 16px);
  }
</style>
