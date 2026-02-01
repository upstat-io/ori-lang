import type { RunResult, FormatResult } from './types';
// Import the WASM JS module - Vite will bundle this properly
import * as wasmBindings from '../../wasm/ori_playground_wasm.js';
// Import the WASM binary URL - Vite handles this with ?url
import wasmBinaryUrl from '../../wasm/ori_playground_wasm_bg.wasm?url';

let wasmModule: {
  run_ori: (code: string) => string;
  format_ori: (code: string, max_width?: number) => string;
  version: () => string;
} | null = null;

let initPromise: Promise<boolean> | null = null;

/**
 * Reset the WASM module to force a reload on next init.
 * Call this after rebuilding WASM to pick up changes.
 */
export function resetWasm(): void {
  wasmModule = null;
  initPromise = null;
}

export async function initWasm(): Promise<boolean> {
  if (wasmModule) return true;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    try {
      // Initialize the WASM module with the binary URL
      // The default export is the init function that loads the .wasm file
      await (wasmBindings as any).default(wasmBinaryUrl);
      wasmModule = wasmBindings as any;
      return true;
    } catch (e: any) {
      console.error('Failed to load WASM:', e);
      console.error('Error details:', e?.message, e?.stack);
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
        error: 'WASM module not loaded.\n\nBuild with:\ncd website/playground-wasm && wasm-pack build --target web --out-dir pkg',
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
