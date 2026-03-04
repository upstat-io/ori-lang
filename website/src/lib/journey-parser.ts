/**
 * Journey markdown parser — reads results markdown and extracts structured
 * data for the interactive journey shell. Called from Astro frontmatter
 * (server-side) so it works in both dev and build modes.
 *
 * Pattern follows roadmap/plan-data.ts: direct fs reads, no prebuild step.
 */

import { readFileSync, readdirSync } from 'fs';
import { join } from 'path';
import type {
  JourneyData,
  JourneyMeta,
  ExecutionResult,
  LexerData,
  ParserData,
  TypeCheckerData,
  ArcData,
  InterpreterData,
  LlvmData,
  ScrutinyCategory,
  Finding,
  ScoreBreakdown,
  AstNode,
} from '../components/journey/journey-types';

const JOURNEYS_DIR = join(process.cwd(), '..', 'plans', 'code-journeys');

/** List all available journey slugs by scanning the directory. */
export function listJourneySlugs(): string[] {
  try {
    return readdirSync(JOURNEYS_DIR)
      .filter(f => /^\d{2}-.*-results\.md$/.test(f))
      .map(f => {
        const content = readFileSync(join(JOURNEYS_DIR, f), 'utf-8');
        const slugMatch = content.match(/^slug:\s*(.+)$/m);
        return slugMatch ? slugMatch[1].trim() : '';
      })
      .filter(Boolean);
  } catch {
    return [];
  }
}

/** Find the file path for a given journey slug. */
function findJourneyFile(slug: string): string | null {
  try {
    const files = readdirSync(JOURNEYS_DIR).filter(f => /^\d{2}-.*-results\.md$/.test(f));
    for (const f of files) {
      const content = readFileSync(join(JOURNEYS_DIR, f), 'utf-8');
      if (content.match(new RegExp(`^slug:\\s*${slug}\\s*$`, 'm'))) {
        return join(JOURNEYS_DIR, f);
      }
    }
  } catch { /* empty */ }
  return null;
}

/** Parse a journey results markdown file into structured JourneyData. */
export function parseJourney(slug: string): JourneyData | null {
  const filePath = findJourneyFile(slug);
  if (!filePath) return null;

  const raw = readFileSync(filePath, 'utf-8');
  const { frontmatter, body } = splitFrontmatter(raw);

  const meta = parseMeta(frontmatter);
  const source = extractCodeBlock(body, 'Source', 'ori');
  const executionResults = parseExecutionResults(body);
  const lexer = parseLexer(body);
  const parser = parseParser(body);
  const typeChecker = parseTypeChecker(body);
  const arc = parseArc(body);
  const interpreter = parseInterpreter(body);
  const llvm = parseLlvm(body);
  const scrutiny = parseScrutiny(body);
  const findings = parseFindings(body);
  const score = parseScore(body, frontmatter);
  const verdict = parseVerdict(body);

  return {
    meta,
    source,
    executionResults,
    lexer,
    parser,
    typeChecker,
    arc,
    interpreter,
    llvm,
    scrutiny,
    findings,
    score,
    verdict,
  };
}

// --- Internal helpers ---

function splitFrontmatter(raw: string): { frontmatter: Record<string, any>; body: string } {
  const match = raw.match(/^---\n([\s\S]*?)\n---\n([\s\S]*)$/);
  if (!match) return { frontmatter: {}, body: raw };

  const fm: Record<string, any> = {};
  let currentKey = '';
  let currentArray: any[] | null = null;

  for (const line of match[1].split('\n')) {
    const arrayItem = line.match(/^\s+-\s+"?([^"]*)"?\s*$/);
    if (arrayItem && currentKey) {
      if (!currentArray) {
        currentArray = [];
        fm[currentKey] = currentArray;
      }
      currentArray.push(arrayItem[1]);
      continue;
    }

    const objItem = line.match(/^\s+(\w+):\s*(.+)$/);
    if (objItem && currentKey && typeof fm[currentKey] === 'object' && !Array.isArray(fm[currentKey])) {
      fm[currentKey][objItem[1]] = parseValue(objItem[2]);
      continue;
    }

    const kv = line.match(/^(\w+):\s*(.*)$/);
    if (kv) {
      currentKey = kv[1];
      currentArray = null;
      const val = kv[2].trim();
      if (val === '') {
        // Could be start of array or object
        fm[currentKey] = [];
      } else {
        fm[currentKey] = parseValue(val);
      }
    }
  }

  // Fix score_breakdown to be an object
  if (fm.score_breakdown && Array.isArray(fm.score_breakdown)) {
    fm.score_breakdown = {};
  }

  // Re-parse score_breakdown properly
  const sbMatch = match[1].match(/score_breakdown:\n((?:\s+\w+:\s*\d+\n?)+)/);
  if (sbMatch) {
    const sb: Record<string, number> = {};
    for (const line of sbMatch[1].split('\n')) {
      const m = line.match(/^\s+(\w+):\s*(\d+)/);
      if (m) sb[m[1]] = parseInt(m[2], 10);
    }
    fm.score_breakdown = sb;
  }

  // Re-parse bugs_found properly
  const bugsSection = match[1].match(/bugs_found:\s*\n((?:\s+-[\s\S]*?)?)(?=\n\w|\n*$)/);
  if (bugsSection && bugsSection[1].trim()) {
    // Parse YAML-style array of objects
    fm.bugs_found = parseYamlArrayOfObjects(bugsSection[1]);
  } else {
    fm.bugs_found = fm.bugs_found || [];
    if (!Array.isArray(fm.bugs_found)) fm.bugs_found = [];
  }

  return { frontmatter: fm, body: match[2] };
}

function parseYamlArrayOfObjects(text: string): Record<string, any>[] {
  const items: Record<string, any>[] = [];
  let current: Record<string, any> | null = null;

  for (const line of text.split('\n')) {
    const newItem = line.match(/^\s+-\s+(\w+):\s*(.+)$/);
    const contItem = line.match(/^\s+(\w+):\s*(.+)$/);

    if (newItem) {
      if (current) items.push(current);
      current = { [newItem[1]]: parseValue(newItem[2]) };
    } else if (contItem && current) {
      current[contItem[1]] = parseValue(contItem[2]);
    }
  }
  if (current) items.push(current);
  return items;
}

function parseValue(s: string): any {
  s = s.trim();
  if (s === '[]') return [];
  if (s.startsWith('"') && s.endsWith('"')) return s.slice(1, -1);
  if (s === 'true') return true;
  if (s === 'false') return false;
  const n = Number(s);
  if (!isNaN(n) && s !== '') return n;
  return s;
}

function parseMeta(fm: Record<string, any>): JourneyMeta {
  return {
    journey: fm.journey ?? 0,
    slug: fm.slug ?? '',
    theme: fm.theme ?? '',
    date: fm.date ?? '',
    status: fm.status ?? '',
    expected: fm.expected ?? 0,
    evalResult: fm.eval_result ?? 0,
    aotResult: fm.aot_result ?? 0,
    difficulty: fm.difficulty ?? 'simple',
    prerequisites: fm.prerequisites ?? [],
    learningObjectives: fm.learning_objectives ?? [],
    features: fm.features ?? [],
    featureDescription: fm.feature_description ?? '',
    overflowCheck: fm.overflow_check ?? '',
  };
}

/** Extract a fenced code block under a specific heading. */
function extractCodeBlock(body: string, heading: string, lang?: string): string {
  const langPattern = lang ? lang : '[a-z]*';
  const pattern = new RegExp(
    `##?#?\\s+${heading}[\\s\\S]*?\`\`\`${langPattern}\\n([\\s\\S]*?)\`\`\``,
    'm'
  );
  const match = body.match(pattern);
  return match ? match[1].trim() : '';
}

/** Extract the text between two headings at the same level. */
function extractSection(body: string, heading: string, level: number = 3): string {
  const hashes = '#'.repeat(level);
  const pattern = new RegExp(
    `${hashes}\\s+${heading}\\b[\\s\\S]*?(?=\\n${hashes}\\s|\\n#{1,${level}}\\s|$)`,
    'm'
  );
  const match = body.match(pattern);
  return match ? match[0] : '';
}

/** Extract blockquote intro from a section. */
function extractIntro(section: string): string {
  const match = section.match(/>\s*([\s\S]*?)(?:\n\n|\n\*\*)/);
  if (!match) return '';
  return match[1].replace(/>\s*/g, '').trim();
}

/** Parse metrics from bold key-value pairs like **Tokens**: 75 */
function parseMetrics(section: string): Record<string, number> {
  const metrics: Record<string, number> = {};
  const pattern = /\*\*([^*]+)\*\*:\s*(\d+)/g;
  let m;
  while ((m = pattern.exec(section)) !== null) {
    metrics[m[1].trim()] = parseInt(m[2], 10);
  }
  return metrics;
}

function parseExecutionResults(body: string): ExecutionResult[] {
  const section = extractSection(body, 'Execution Results', 2);
  if (!section) return [];

  const results: ExecutionResult[] = [];
  const rows = section.match(/\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|/g) || [];

  for (const row of rows.slice(2)) { // Skip header and separator
    const cells = row.split('|').map(c => c.trim()).filter(Boolean);
    if (cells.length >= 6) {
      results.push({
        backend: cells[0],
        exitCode: parseInt(cells[1], 10) || 0,
        expected: parseInt(cells[2], 10) || 0,
        stdout: cells[3] === '(none)' ? '' : cells[3],
        stderr: cells[4] === '(none)' ? '' : cells[4],
        status: cells[5],
      });
    }
  }
  return results;
}

function parseLexer(body: string): LexerData {
  const section = extractSection(body, '1\\.\\s*Lexer') || extractSection(body, 'Lexer', 3);
  return {
    intro: extractIntro(section),
    metrics: parseMetrics(section),
    tokens: parseTokenStream(section),
  };
}

function parseTokenStream(section: string): { type: string; value?: string }[] {
  const codeMatch = section.match(/```text\n([\s\S]*?)```/);
  if (!codeMatch) return [];

  const tokens: { type: string; value?: string }[] = [];
  const raw = codeMatch[1].trim();

  for (const token of raw.split(/\s+/)) {
    const withValue = token.match(/^(\w+)\(([^)]*)\)$/);
    if (withValue) {
      tokens.push({ type: withValue[1], value: withValue[2] });
    } else if (token.trim()) {
      tokens.push({ type: token.trim() });
    }
  }
  return tokens;
}

function parseParser(body: string): ParserData {
  const section = extractSection(body, '2\\.\\s*Parser') || extractSection(body, 'Parser', 3);
  const astText = extractCodeBlock(section, 'AST.*', 'text') ||
    (() => { const m = section.match(/```text\n([\s\S]*?)```/); return m ? m[1].trim() : ''; })();

  return {
    intro: extractIntro(section),
    metrics: parseMetrics(section),
    astText,
    nodes: parseAstText(astText),
  };
}

function parseAstText(text: string): AstNode[] {
  if (!text) return [];

  const lines = text.split('\n');
  const root: AstNode = { label: 'root', depth: -1, children: [] };
  const stack: AstNode[] = [root];

  for (const line of lines) {
    if (!line.trim()) continue;

    // Calculate depth from tree-drawing characters
    const stripped = line.replace(/^[│├└─\s]+/, '');
    const prefixLen = line.length - line.replace(/^[│├└─\s]+/, '').length;
    const depth = Math.floor(prefixLen / 3);

    const node: AstNode = { label: stripped.trim(), depth, children: [] };

    // Find parent at correct depth
    while (stack.length > 1 && stack[stack.length - 1].depth >= depth) {
      stack.pop();
    }

    stack[stack.length - 1].children.push(node);
    stack.push(node);
  }

  return root.children;
}

function parseTypeChecker(body: string): TypeCheckerData {
  const section = extractSection(body, '3\\.\\s*Type Checker') || extractSection(body, 'Type Checker', 3);
  const annotatedSource = extractCodeBlock(section, 'Inferred types', 'ori') ||
    (() => { const m = section.match(/```ori\n([\s\S]*?)```/); return m ? m[1].trim() : ''; })();

  return {
    intro: extractIntro(section),
    metrics: parseMetrics(section),
    annotatedSource,
  };
}

function parseArc(body: string): ArcData {
  // Look for ARC Pipeline section (could be at various heading levels)
  const section = extractSection(body, '5\\.\\s*ARC Pipeline') ||
    extractSection(body, 'ARC Pipeline', 3) ||
    extractSection(body, 'ARC Pipeline', 4);

  const arcAnnotations = extractCodeBlock(section, 'ARC annotations', 'text') ||
    (() => { const m = section.match(/```text\n([\s\S]*?)```/); return m ? m[1].trim() : ''; })();

  const functions = parseArcFunctions(arcAnnotations);

  return {
    intro: extractIntro(section),
    metrics: parseMetrics(section),
    functions,
    annotations: arcAnnotations,
  };
}

function parseArcFunctions(annotations: string): { name: string; rcInc: number; rcDec: number; balanced: boolean; notes: string }[] {
  const fns: { name: string; rcInc: number; rcDec: number; balanced: boolean; notes: string }[] = [];
  const pattern = /(@\w+):\s*\+?(\d+)\s*rc_inc,?\s*\+?(\d+)\s*rc_dec\s*(?:\(([^)]*)\))?/g;
  let m;
  while ((m = pattern.exec(annotations)) !== null) {
    const rcInc = parseInt(m[2], 10);
    const rcDec = parseInt(m[3], 10);
    fns.push({
      name: m[1],
      rcInc,
      rcDec,
      balanced: rcInc === rcDec,
      notes: m[4] || '',
    });
  }
  return fns;
}

function parseInterpreter(body: string): InterpreterData {
  const section = extractSection(body, 'Backend:\\s*Interpreter') ||
    extractSection(body, 'Interpreter', 3);

  const resultMatch = section.match(/\*\*Result\*\*:\s*(\d+)/);
  const statusMatch = section.match(/\*\*Status\*\*:\s*(\w+)/);
  const traceMatch = section.match(/```text\n([\s\S]*?)```/);

  return {
    intro: extractIntro(section),
    result: resultMatch ? parseInt(resultMatch[1], 10) : 0,
    status: statusMatch ? statusMatch[1] : 'UNKNOWN',
    trace: traceMatch ? traceMatch[1].trim() : '',
  };
}

function parseLlvm(body: string): LlvmData {
  const llvmSection = extractSection(body, 'Backend:\\s*LLVM') ||
    extractSection(body, 'LLVM Codegen', 3);

  // ARC annotations within LLVM section
  const arcInLlvm = extractSection(body, 'ARC Pipeline', 4);
  const arcAnnotations = extractCodeBlock(arcInLlvm || llvmSection, 'ARC annotations', 'text') ||
    (() => { const m = (arcInLlvm || llvmSection).match(/```text\n([\s\S]*?)```/); return m ? m[1].trim() : ''; })();

  // Generated LLVM IR
  const irMatch = body.match(/####?\s+Generated LLVM IR[\s\S]*?```llvm\n([\s\S]*?)```/);
  const ir = irMatch ? irMatch[1].trim() : '';

  // Assembly
  const asmMatch = body.match(/####?\s+Disassembly[\s\S]*?```asm\n([\s\S]*?)```/);
  const assembly = asmMatch ? asmMatch[1].trim() : '';

  // Ideal vs Actual
  const idealVsActual = parseIdealVsActual(body);

  // Binary metrics
  const binarySection = extractSection(body, 'Binary Analysis', 3) ||
    extractSection(body, '6\\.\\s*Binary Analysis', 3);
  const binaryMetrics: Record<string, string> = {};
  const metricPattern = /\|\s*([^|]+)\s*\|\s*([^|]+)\s*\|/g;
  let bm;
  const binaryRows = binarySection.match(/\|[^|]+\|[^|]+\|/g) || [];
  for (const row of binaryRows.slice(2)) {
    const cells = row.split('|').map(c => c.trim()).filter(Boolean);
    if (cells.length >= 2) {
      binaryMetrics[cells[0]] = cells[1];
    }
  }

  return {
    intro: extractIntro(llvmSection),
    arcAnnotations,
    ir,
    idealVsActual,
    assembly,
    binaryMetrics,
  };
}

function parseIdealVsActual(body: string): { fn: string; ideal: string; actual: string; idealCount: number; actualCount: number; verdict: string }[] {
  const results: { fn: string; ideal: string; actual: string; idealCount: number; actualCount: number; verdict: string }[] = [];

  // Find the "Optimal IR Comparison" or "Ideal vs Actual" sections
  const compSection = extractSection(body, 'Optimal IR Comparison', 3) ||
    extractSection(body, '7\\.\\s*Optimal IR Comparison', 3);
  if (!compSection) return results;

  // Find per-function comparisons (#### @fn: Ideal vs Actual)
  const fnSections = compSection.split(/####\s+/).filter(s => s.includes('Ideal vs Actual'));

  for (const fnSection of fnSections) {
    const fnName = fnSection.match(/^(@?\w+)/)?.[1] || '';
    const llvmBlocks = fnSection.match(/```llvm\n([\s\S]*?)```/g) || [];
    const ideal = llvmBlocks[0]?.replace(/```llvm\n/, '').replace(/```/, '').trim() || '';
    const actual = llvmBlocks[1]?.replace(/```llvm\n/, '').replace(/```/, '').trim() || '';

    const idealCount = ideal.split('\n').filter(l => l.trim() && !l.trim().startsWith(';') && !l.trim().startsWith('}')).length;
    const actualCount = actual.split('\n').filter(l => l.trim() && !l.trim().startsWith(';') && !l.trim().startsWith('}')).length;

    const deltaMatch = fnSection.match(/\*\*Delta\*\*:\s*\+?(\d+)/);
    const verdictMatch = fnSection.match(/\*\*(OPTIMAL|NEAR-OPTIMAL|SUBOPTIMAL)\*\*/);

    results.push({
      fn: fnName,
      ideal,
      actual,
      idealCount,
      actualCount,
      verdict: verdictMatch ? verdictMatch[1] : (idealCount === actualCount ? 'OPTIMAL' : 'NEAR-OPTIMAL'),
    });
  }

  // Also check the module summary table
  if (results.length === 0) {
    const summaryTable = compSection.match(/\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|[^|]+\|/g) || [];
    for (const row of summaryTable.slice(2)) {
      const cells = row.split('|').map(c => c.trim()).filter(Boolean);
      if (cells.length >= 5) {
        results.push({
          fn: cells[0],
          ideal: '',
          actual: '',
          idealCount: parseInt(cells[1], 10) || 0,
          actualCount: parseInt(cells[2], 10) || 0,
          verdict: cells[4] || '',
        });
      }
    }
  }

  return results;
}

function parseScrutiny(body: string): ScrutinyCategory[] {
  const categories: ScrutinyCategory[] = [];
  const scrutinyStart = body.indexOf('## Deep Scrutiny');
  if (scrutinyStart === -1) return categories;

  const scrutinyEnd = body.indexOf('## Findings', scrutinyStart);
  const scrutinyBody = scrutinyEnd !== -1
    ? body.slice(scrutinyStart, scrutinyEnd)
    : body.slice(scrutinyStart);

  // Split by ### headings
  const sections = scrutinyBody.split(/###\s+\d+\.\s+/).filter(s => s.trim());

  for (const section of sections.slice(0)) { // skip the header
    const nameMatch = section.match(/^([^\n]+)/);
    if (!nameMatch) continue;

    const name = nameMatch[1].trim();
    const table = parseTable(section);
    const prose = section
      .replace(/^[^\n]+\n/, '') // remove heading
      .replace(/\|[\s\S]*?\n\n/, '') // remove table
      .trim();

    categories.push({ name, table, prose });
  }

  return categories;
}

function parseTable(text: string): string[][] {
  const rows: string[][] = [];
  const tableMatch = text.match(/(\|[^\n]+\|(?:\n\|[^\n]+\|)*)/);
  if (!tableMatch) return rows;

  const lines = tableMatch[1].split('\n');
  for (const line of lines) {
    if (line.includes('---')) continue; // separator row
    const cells = line.split('|').map(c => c.trim()).filter(Boolean);
    if (cells.length > 0) rows.push(cells);
  }
  return rows;
}

function parseFindings(body: string): Finding[] {
  const findingsStart = body.indexOf('## Findings');
  if (findingsStart === -1) return [];

  const findingsEnd = body.indexOf('## Codegen Quality Score', findingsStart);
  const findingsBody = findingsEnd !== -1
    ? body.slice(findingsStart, findingsEnd)
    : body.slice(findingsStart);

  const findings: Finding[] = [];

  // Parse the summary table
  const tableRows = findingsBody.match(/\|\s*\d+\s*\|[^\n]+/g) || [];
  for (const row of tableRows) {
    const cells = row.split('|').map(c => c.trim()).filter(Boolean);
    if (cells.length >= 6) {
      const id = cells[0];
      findings.push({
        id,
        severity: cells[1],
        category: cells[2],
        description: cells[3],
        status: cells[4],
        detail: '',
      });
    }
  }

  // Parse detailed finding descriptions
  const detailSections = findingsBody.match(/###\s+\w+-\d+:[\s\S]*?(?=###\s+\w+-\d+:|## |$)/g) || [];
  for (const detail of detailSections) {
    const idMatch = detail.match(/###\s+(\w+-\d+):/);
    if (!idMatch) continue;

    const numId = idMatch[1].replace(/\w+-/, '');
    const finding = findings.find(f => f.id === numId);
    if (finding) {
      finding.detail = detail.replace(/###\s+\w+-\d+:[^\n]*\n/, '').trim();
    }
  }

  return findings;
}

function parseScore(body: string, frontmatter: Record<string, any>): { overall: number; breakdown: ScoreBreakdown[] } {
  const scoreSection = extractSection(body, 'Codegen Quality Score', 2);
  const breakdown: ScoreBreakdown[] = [];

  const rows = scoreSection.match(/\|\s*[^|]+\|\s*\d+%\s*\|[^\n]+/g) || [];
  for (const row of rows) {
    const cells = row.split('|').map(c => c.trim()).filter(Boolean);
    if (cells.length >= 4) {
      const scoreMatch = cells[2].match(/(\d+)/);
      const weightMatch = cells[1].match(/(\d+)/);
      breakdown.push({
        category: cells[0],
        weight: weightMatch ? parseInt(weightMatch[1], 10) : 0,
        score: scoreMatch ? parseInt(scoreMatch[1], 10) : 0,
        notes: cells[3] || '',
      });
    }
  }

  const overallMatch = scoreSection.match(/\*\*Overall:\s*([\d.]+)/);
  const overall = overallMatch ? parseFloat(overallMatch[1]) : (frontmatter.score ?? 0);

  return { overall, breakdown };
}

function parseVerdict(body: string): string {
  const verdictSection = extractSection(body, 'Verdict', 2);
  if (!verdictSection) return '';

  return verdictSection
    .replace(/^##\s+Verdict\s*\n/, '')
    .trim();
}
