<script lang="ts">
  import type { LexerData } from '../journey-types';
  import PhaseIntro from '../shared/PhaseIntro.svelte';
  import TokenStream from '../shared/TokenStream.svelte';

  let { lexer }: { lexer: LexerData } = $props();
</script>

<div class="lexer-phase">
  <h2>Lexer</h2>

  <PhaseIntro intro={lexer.intro} metrics={lexer.metrics} />

  <div class="token-section">
    <h3>Token Stream</h3>
    <div class="token-container">
      <TokenStream tokens={lexer.tokens} />
    </div>
  </div>
</div>

<style>
  .lexer-phase {
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

  .token-section {
    margin-top: var(--space-4, 16px);
  }

  .token-container {
    border: 1px solid var(--color-border, #333333);
    border-radius: var(--radius-md, 8px);
    background: var(--color-bg-code, #111111);
    overflow: hidden;
  }
</style>
