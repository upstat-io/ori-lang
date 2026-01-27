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
</style>
