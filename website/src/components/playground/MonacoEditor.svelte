<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type * as Monaco from 'monaco-editor';

  let {
    value = $bindable(''),
    fontSize = 14,
    onrun,
  }: {
    value: string;
    fontSize?: number;
    onrun?: () => void;
  } = $props();

  let containerEl: HTMLDivElement;
  let editor = $state<Monaco.editor.IStandaloneCodeEditor | undefined>(undefined);
  let monaco: typeof Monaco | undefined;

  // Sync external value changes into editor
  // Both `editor` and `value` must be read to track dependencies properly
  $effect(() => {
    const currentEditor = editor;
    const currentValue = value;
    if (currentEditor && currentValue !== currentEditor.getValue()) {
      currentEditor.setValue(currentValue);
    }
  });

  onMount(async () => {
    // Dynamic import to avoid SSR issues
    monaco = await import('monaco-editor');

    const { oriMonarchTokens, oriLanguageConfig } = await import('./ori-monarch');
    const { oriDarkTheme } = await import('./ori-theme');

    monaco.languages.register({ id: 'ori' });
    monaco.languages.setMonarchTokensProvider('ori', oriMonarchTokens);
    monaco.languages.setLanguageConfiguration('ori', oriLanguageConfig);
    monaco.editor.defineTheme('ori-dark', oriDarkTheme);

    editor = monaco.editor.create(containerEl, {
      value,
      language: 'ori',
      theme: 'ori-dark',
      fontSize,
      fontFamily: "'JetBrains Mono', 'Consolas', 'Monaco', monospace",
      fontLigatures: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      automaticLayout: true,
      tabSize: 4,
      insertSpaces: true,
      renderWhitespace: 'selection',
      wordWrap: 'on',
      lineNumbers: 'on',
      glyphMargin: false,
      folding: true,
      lineDecorationsWidth: 10,
      lineNumbersMinChars: 3,
      // Disable rainbow bracket colorization
      'bracketPairColorization.enabled': false,
      matchBrackets: 'always',
    });

    // Sync editor changes to bindable value
    editor.onDidChangeModelContent(() => {
      if (editor) {
        value = editor.getValue();
      }
    });

    // Ctrl+Enter to run
    if (onrun) {
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
        onrun?.();
      });
    }
  });

  onDestroy(() => {
    editor?.dispose();
  });
</script>

<div class="monaco-container" bind:this={containerEl}></div>

<style>
  .monaco-container {
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  /* Override Monaco's bracket matching - subtle blue accent */
  .monaco-container :global(.monaco-editor .bracket-match),
  .monaco-container :global(.monaco-editor .matchingBracket),
  .monaco-container :global(.bracket-match) {
    background-color: rgba(86, 156, 214, 0.2) !important;
    border: 1px solid rgba(86, 156, 214, 0.6) !important;
    box-sizing: border-box;
  }

  /* Override Monaco's selection colors */
  .monaco-container :global(.monaco-editor .selected-text),
  .monaco-container :global(.monaco-editor .selectionHighlight),
  .monaco-container :global(.monaco-editor .selection),
  .monaco-container :global(.monaco-editor .focused .selected-text),
  .monaco-container :global(.monaco-editor .view-overlays .selected-text) {
    background-color: rgba(86, 156, 214, 0.3) !important;
  }

  .monaco-container :global(.monaco-editor .wordHighlight),
  .monaco-container :global(.monaco-editor .wordHighlightStrong),
  .monaco-container :global(.monaco-editor .wordHighlightText) {
    background-color: rgba(86, 156, 214, 0.2) !important;
    border: none !important;
  }

  .monaco-container :global(.monaco-editor .findMatch),
  .monaco-container :global(.monaco-editor .currentFindMatch) {
    background-color: rgba(86, 156, 214, 0.4) !important;
    border: none !important;
  }

  /* Selection highlight for matching occurrences */
  .monaco-container :global(.monaco-editor .selectionHighlight),
  .monaco-container :global(.monaco-editor .selection-highlight),
  .monaco-container :global(.monaco-editor .focused .selectionHighlight),
  .monaco-container :global(.monaco-editor .view-overlays .selectionHighlight) {
    background-color: rgba(86, 156, 214, 0.15) !important;
    border: none !important;
  }

  /* Symbol occurrences highlighting */
  .monaco-container :global(.monaco-editor .occurrencesHighlight),
  .monaco-container :global(.monaco-editor .documentHighlight) {
    background-color: rgba(86, 156, 214, 0.2) !important;
    border: none !important;
  }

  /* Scrollbar CSS override - required due to Monaco bug #2650
     Theme colors don't apply to scrollbar, only minimap */
  .monaco-container :global(.monaco-editor .monaco-scrollable-element > .scrollbar > .slider) {
    background: rgba(99, 104, 116, 0.3) !important;
  }
  .monaco-container :global(.monaco-editor .monaco-scrollable-element > .scrollbar > .slider:hover) {
    background: rgba(99, 104, 116, 0.5) !important;
  }
  .monaco-container :global(.monaco-editor .monaco-scrollable-element > .scrollbar > .slider.active) {
    background: rgba(99, 104, 116, 0.7) !important;
  }
</style>
