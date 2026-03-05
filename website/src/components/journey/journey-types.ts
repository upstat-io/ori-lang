/** Structured data for a Code Journey, parsed from results markdown. */

export interface JourneyMeta {
  journey: number;
  slug: string;
  theme: string;
  date: string;
  status: string;
  expected: number;
  evalResult: number;
  aotResult: number;
  difficulty: 'simple' | 'moderate' | 'complex';
  prerequisites: string[];
  learningObjectives: string[];
  features: string[];
  featureDescription: string;
  overflowCheck: string;
}

export interface ExecutionResult {
  backend: string;
  exitCode: number;
  expected: number;
  stdout: string;
  stderr: string;
  status: string;
}

export interface LexerData {
  intro: string;
  metrics: Record<string, number>;
  tokens: { type: string; value?: string }[];
}

export interface AstNode {
  label: string;
  depth: number;
  children: AstNode[];
}

export interface ParserData {
  intro: string;
  metrics: Record<string, number>;
  astText: string;
  nodes: AstNode[];
}

export interface TypeCheckerData {
  intro: string;
  metrics: Record<string, number>;
  annotatedSource: string;
}

export interface ArcFunction {
  name: string;
  rcInc: number;
  rcDec: number;
  balanced: boolean;
  notes: string;
}

export interface ArcData {
  intro: string;
  metrics: Record<string, number>;
  functions: ArcFunction[];
  annotations: string;
}

export interface InterpreterData {
  intro: string;
  result: number;
  status: string;
  trace: string;
}

export interface IdealVsActual {
  fn: string;
  ideal: string;
  actual: string;
  idealCount: number;
  actualCount: number;
  verdict: string;
}

export interface LlvmData {
  intro: string;
  arcAnnotations: string;
  ir: string;
  idealVsActual: IdealVsActual[];
  assembly: string;
  binaryMetrics: Record<string, string>;
}

export interface ScrutinyCategory {
  name: string;
  table: string[][];
  prose: string;
}

export interface Finding {
  id: string;
  severity: string;
  category: string;
  description: string;
  status: string;
  detail: string;
}

export interface ScoreBreakdown {
  category: string;
  weight: number;
  score: number;
  notes: string;
}

export interface JourneyData {
  meta: JourneyMeta;
  source: string;
  executionResults: ExecutionResult[];
  lexer: LexerData;
  parser: ParserData;
  typeChecker: TypeCheckerData;
  arc: ArcData;
  interpreter: InterpreterData;
  llvm: LlvmData;
  scrutiny: ScrutinyCategory[];
  findings: Finding[];
  score: { overall: number; breakdown: ScoreBreakdown[] };
  verdict: string;
}

/** Phase identifiers for navigation */
export type PhaseId = 'source' | 'lexer' | 'parser' | 'typechecker' | 'arc' | 'eval' | 'llvm' | 'score';

export const PHASES: { id: PhaseId; name: string; index: number }[] = [
  { id: 'source', name: 'Source', index: 0 },
  { id: 'lexer', name: 'Lexer', index: 1 },
  { id: 'parser', name: 'Parser', index: 2 },
  { id: 'typechecker', name: 'Type Checker', index: 3 },
  { id: 'arc', name: 'ARC', index: 4 },
  { id: 'eval', name: 'Eval', index: 5 },
  { id: 'llvm', name: 'LLVM', index: 6 },
  { id: 'score', name: 'Score', index: 7 },
];
