export interface RunResult {
  success: boolean;
  output?: string;
  printed?: string;
  error?: string;
  error_type?: 'parse' | 'type' | 'runtime';
}

export interface PlaygroundConfig {
  showToolbar: boolean;
  showOutput: boolean;
  height: string;
  enableShare: boolean;
  enableExamples: boolean;
  readUrlHash: boolean;
  initialCode?: string;
  fontSize: number;
  layout: 'horizontal' | 'vertical';
}

export const DEFAULT_CONFIG: PlaygroundConfig = {
  showToolbar: true,
  showOutput: true,
  height: '100%',
  enableShare: true,
  enableExamples: true,
  readUrlHash: true,
  fontSize: 14,
  layout: 'horizontal',
};
