<script lang="ts">
  import { onMount } from 'svelte';
  import MonacoEditor from './MonacoEditor.svelte';
  import OutputPane from './OutputPane.svelte';
  import PlaygroundToolbar from './PlaygroundToolbar.svelte';
  import { EXAMPLES } from './examples';
  import { initWasm, runOri, isReady, getVersion } from './wasm-runner';
  import { DEFAULT_CONFIG, type PlaygroundConfig, type RunResult } from './types';

  let { config = {} }: { config?: Partial<PlaygroundConfig> } = $props();

  const cfg: PlaygroundConfig = { ...DEFAULT_CONFIG, ...config };

  // Default to 'hello' example unless custom initialCode is provided
  let selectedExample = $state(cfg.initialCode ? '' : 'hello');
  let code = $state(cfg.initialCode ?? EXAMPLES.hello.code);
  let result: RunResult | null = $state(null);
  let elapsed = $state('');
  let status: 'idle' | 'running' | 'success' | 'error' = $state('idle');
  let shareLabel = $state('Share');
  let wasmVersion = $state('Loading...');

  onMount(async () => {
    // Read URL hash if enabled
    if (cfg.readUrlHash && window.location.hash) {
      try {
        code = decodeURIComponent(atob(window.location.hash.slice(1)));
        selectedExample = ''; // Clear selection when loading from URL
      } catch {
        // Ignore invalid hash
      }
    }

    const ready = await initWasm();
    if (ready) {
      wasmVersion = getVersion();
    } else {
      wasmVersion = 'WASM not loaded';
      result = {
        success: false,
        error: 'Failed to load WASM module.\n\nBuild with:\ncd playground/wasm && wasm-pack build --target web --out-dir ../pkg',
        error_type: 'runtime',
      };
      status = 'error';
    }
  });

  async function handleRun() {
    if (!isReady()) return;

    status = 'running';
    result = null;
    elapsed = '';

    // Let UI update
    await new Promise((r) => setTimeout(r, 10));

    try {
      const res = runOri(code);
      result = res.result;
      elapsed = res.elapsed;
      status = result.success ? 'success' : 'error';
    } catch (e: any) {
      result = {
        success: false,
        error: `Internal error: ${e.message}`,
        error_type: 'runtime',
      };
      status = 'error';
    }
  }

  function handleShare() {
    const encoded = btoa(encodeURIComponent(code));
    const url = `${window.location.origin}/playground#${encoded}`;

    navigator.clipboard.writeText(url).then(() => {
      shareLabel = 'Copied!';
      setTimeout(() => { shareLabel = 'Share'; }, 2000);
    }).catch(() => {
      prompt('Copy this URL:', url);
    });
  }

  function handleExample(name: string) {
    const example = EXAMPLES[name];
    if (example) {
      selectedExample = name;
      code = example.code;
    }
  }
</script>

<div class="playground" style="height: {cfg.height};" class:horizontal={cfg.layout === 'horizontal'} class:vertical={cfg.layout === 'vertical'}>
  {#if cfg.showToolbar}
    <PlaygroundToolbar
      enableShare={cfg.enableShare}
      enableExamples={cfg.enableExamples}
      running={status === 'running'}
      {selectedExample}
      {shareLabel}
      onrun={handleRun}
      onshare={handleShare}
      onexample={handleExample}
    />
  {/if}

  <div class="playground-body">
    <div class="editor-pane">
      {#if !cfg.showToolbar}
        <div class="pane-header">
          <span>main.ori</span>
        </div>
      {/if}
      <MonacoEditor bind:value={code} fontSize={cfg.fontSize} onrun={handleRun} />
    </div>

    {#if cfg.showOutput}
      <OutputPane {result} {elapsed} {status} />
    {/if}
  </div>

  <div class="playground-footer">
    <span>{wasmVersion}</span>
  </div>
</div>

<style>
  .playground {
    display: flex;
    flex-direction: column;
    width: 100%;
    background: var(--color-bg-code, #1e1e1e);
    border: 1px solid var(--color-border, #2a2b35);
    border-radius: var(--radius-lg, 8px);
    overflow: hidden;
  }

  .playground-body {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .horizontal .playground-body {
    flex-direction: row;
  }

  .vertical .playground-body {
    flex-direction: column;
  }

  .editor-pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }

  .horizontal .editor-pane {
    border-right: 1px solid var(--color-border, #2a2b35);
  }

  .vertical .editor-pane {
    border-bottom: 1px solid var(--color-border, #2a2b35);
  }

  .pane-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 1rem;
    background: var(--color-bg-elevated, #21222c);
    border-bottom: 1px solid var(--color-border, #2a2b35);
    font-size: 0.8125rem;
    color: var(--color-text-secondary, #9ca0ab);
    flex-shrink: 0;
  }

  .playground-footer {
    display: flex;
    justify-content: space-between;
    padding: 0.375rem 1rem;
    background: var(--color-bg-elevated, #21222c);
    border-top: 1px solid var(--color-border, #2a2b35);
    font-size: 0.75rem;
    color: var(--color-text-muted, #636874);
    flex-shrink: 0;
  }

  @media (max-width: 768px) {
    .horizontal .playground-body {
      flex-direction: column;
    }

    .horizontal .editor-pane {
      border-right: none;
      border-bottom: 1px solid var(--color-border, #2a2b35);
    }
  }
</style>
