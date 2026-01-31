import type { RunResult, FormatResult } from './types';

let wasmModule: {
  run_ori: (code: string) => string;
  format_ori: (code: string, max_width?: number) => string;
  version: () => string;
} | null = null;

let initPromise: Promise<boolean> | null = null;

// Cache buster for dev mode - increment to force reload
let cacheBuster = 0;

/**
 * Reset the WASM module to force a reload on next init.
 * Call this after rebuilding WASM to pick up changes.
 */
export function resetWasm(): void {
  wasmModule = null;
  initPromise = null;
  cacheBuster++;
}

export async function initWasm(): Promise<boolean> {
  if (wasmModule) return true;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // In dev mode, use cache busting to force reload after WASM rebuild
      // The timestamp query param is ignored by Vite but busts browser cache
      const isDev = import.meta.env?.DEV;
      const wasmPath = isDev
        ? `../../wasm/ori_playground_wasm.js?t=${Date.now()}&v=${cacheBuster}`
        : '../../wasm/ori_playground_wasm.js';
      const wasm = await import(/* @vite-ignore */ wasmPath);
      // Explicitly provide path to WASM binary in public folder
      // In dev, it's also served from public. In prod, it's a static asset.
      const wasmBinaryPath = '/wasm/ori_playground_wasm_bg.wasm';
      await wasm.default(wasmBinaryPath);
      wasmModule = wasm;
      return true;
    } catch (e) {
      console.error('Failed to load WASM:', e);
      return false;
    }
  })();

  return initPromise;
}

export function getVersion(): string {
  if (!wasmModule) return 'WASM not loaded';
  return wasmModule.version();
}

export function isReady(): boolean {
  return wasmModule !== null;
}

export function runOri(code: string): { result: RunResult; elapsed: string } {
  if (!wasmModule) {
    return {
      result: {
        success: false,
        error: 'WASM module not loaded.\n\nBuild with:\ncd playground/wasm && wasm-pack build --target web --out-dir ../pkg',
        error_type: 'runtime',
      },
      elapsed: '0ms',
    };
  }

  const startTime = performance.now();
  const resultJson = wasmModule.run_ori(code);
  const elapsedMs = performance.now() - startTime;
  const result: RunResult = JSON.parse(resultJson);

  const elapsedSec = elapsedMs / 1000;
  const elapsed = elapsedSec >= 0.01
    ? `${elapsedSec.toFixed(2)}s`
    : `${elapsedMs.toFixed(1)}ms`;

  return { result, elapsed };
}

export function formatOri(code: string, maxWidth?: number): FormatResult {
  if (!wasmModule) {
    return {
      success: false,
      error: 'WASM module not loaded',
    };
  }

  try {
    const resultJson = wasmModule.format_ori(code, maxWidth);
    return JSON.parse(resultJson);
  } catch (e: any) {
    return {
      success: false,
      error: `Format error: ${e.message}`,
    };
  }
}
