import { readFileSync, readdirSync, existsSync } from 'fs';
import { join, basename } from 'path';

// ============================================================================
// Core Interfaces
// ============================================================================

export interface Task {
  name: string;
  done: boolean;
}

export interface Section {
  name: string;
  tasks: Task[];
}

export interface RoadmapSection {
  num: number | string;
  slug: string;
  name: string;
  status: 'complete' | 'partial' | 'not-started';
  note?: string;
  goal: string;
  spec?: string;
  subsections: Section[];
}

export interface Reroute {
  name: string;
  fullName: string;
  status: 'active' | 'queued' | 'resolved';
  key: string;    // URL-friendly key (hyphens)
  dir: string;    // filesystem directory name
}

// ============================================================================
// Reroute Plan Registry
// ============================================================================

export const reroutes: Reroute[] = [
  {
    name: 'Merkle Pool',
    fullName: 'Merkle Pool Identity',
    status: 'resolved',
    key: 'merkle-pool-identity',
    dir: 'merkle_pool_identity',
  },
  {
    name: 'Diagnostics',
    fullName: 'Compiler Diagnostics Toolkit',
    status: 'resolved',
    key: 'compiler-diagnostics',
    dir: 'compiler-diagnostics',
  },
  {
    name: 'Value Semantics',
    fullName: 'Value Semantics Optimization',
    status: 'active',
    key: 'value-semantics-optimization',
    dir: 'value-semantics-optimization',
  },
  {
    name: 'LLVM Fixes',
    fullName: 'LLVM Codegen Fixes',
    status: 'queued',
    key: 'llvm-codegen-fixes',
    dir: 'llvm-codegen-fixes',
  },
  {
    name: 'EH Personality',
    fullName: 'Ori EH Personality',
    status: 'queued',
    key: 'ori-eh-personality',
    dir: 'ori-eh-personality',
  },
  {
    name: 'Type Registry',
    fullName: 'Type Strategy Registry',
    status: 'queued',
    key: 'type-strategy-registry',
    dir: 'type_strategy_registry',
  },
  {
    name: 'Repr Opt',
    fullName: 'Representation Optimization & ARC Intelligence',
    status: 'queued',
    key: 'repr-opt',
    dir: 'repr-opt',
  },
];

// ============================================================================
// YAML Frontmatter Parser
// ============================================================================

interface YamlSection {
  id: string;
  title: string;
  status: string;
}

export interface YamlFrontmatter {
  section: number | string;
  title: string;
  status: string;
  tier?: number;
  goal: string;
  spec?: string | string[];
  inspired_by?: string[];
  depends_on?: string[];
  sections: YamlSection[];
}

/**
 * Parse simple YAML frontmatter (handles our specific schema).
 * Supports top-level key: value pairs, simple arrays, and arrays of objects.
 */
export function parseYamlFrontmatter(yaml: string): YamlFrontmatter | null {
  const lines = yaml.trim().split('\n');
  const result: Record<string, unknown> = {};
  let currentKey = '';
  let currentArray: unknown[] | null = null;
  let currentObject: Record<string, unknown> | null = null;
  let inArray = false;

  for (const line of lines) {
    if (!line.trim()) continue;

    // Check for array item (starts with "  - ")
    const arrayItemMatch = line.match(/^(\s*)- (.+)$/);
    if (arrayItemMatch) {
      const [, indent, value] = arrayItemMatch;
      const indentLevel = indent.length;

      const objectMatch = value.match(/^(\w+):\s*(.*)$/);
      if (objectMatch && indentLevel >= 2) {
        const [, key, val] = objectMatch;
        if (key === 'id') {
          if (currentObject && currentArray) {
            currentArray.push(currentObject);
          }
          currentObject = { id: val.replace(/^["']|["']$/g, '') };
        } else if (currentObject) {
          currentObject[key] = val.replace(/^["']|["']$/g, '');
        }
      } else if (inArray && currentArray) {
        currentArray.push(value.trim());
      }
      continue;
    }

    // Check for indented object property (continuation of array object)
    const indentedKvMatch = line.match(/^(\s+)(\w+):\s*(.*)$/);
    if (indentedKvMatch && currentObject) {
      const [, indent, key, val] = indentedKvMatch;
      if (indent.length >= 4) {
        currentObject[key] = val.replace(/^["']|["']$/g, '');
        continue;
      }
    }

    // Top-level key: value
    const kvMatch = line.match(/^(\w+):\s*(.*)$/);
    if (kvMatch) {
      if (currentKey && currentArray) {
        if (currentObject) {
          currentArray.push(currentObject);
          currentObject = null;
        }
        result[currentKey] = currentArray;
        currentArray = null;
      }

      const [, key, value] = kvMatch;
      currentKey = key;

      if (value === '' || value === undefined) {
        currentArray = [];
        inArray = true;
      } else if (value === '[]') {
        // Inline empty array syntax
        result[key] = [];
        inArray = false;
      } else if (value.startsWith('[') && value.endsWith(']')) {
        // Inline array syntax: ["a", "b"] or [a, b]
        const inner = value.slice(1, -1).trim();
        if (inner === '') {
          result[key] = [];
        } else {
          result[key] = inner.split(',').map(item =>
            item.trim().replace(/^["']|["']$/g, '')
          );
        }
        inArray = false;
      } else {
        let parsed: unknown = value.replace(/^["']|["']$/g, '');
        if (parsed === 'true') parsed = true;
        else if (parsed === 'false') parsed = false;
        else if (/^\d+$/.test(parsed as string)) parsed = parseInt(parsed as string, 10);
        result[key] = parsed;
        inArray = false;
      }
    }
  }

  if (currentKey && currentArray) {
    if (currentObject) {
      currentArray.push(currentObject);
    }
    result[currentKey] = currentArray;
  }

  // Normalize "subsections" to "sections" (some plan files use either)
  if (!result.sections && result.subsections) {
    result.sections = result.subsections;
    delete result.subsections;
  }

  return result as unknown as YamlFrontmatter;
}

// ============================================================================
// Task Extraction
// ============================================================================

/**
 * Extract tasks from markdown body by parsing checkboxes under section headers.
 * Returns a map of section ID -> tasks.
 */
export function extractTasksFromBody(body: string): Map<string, Task[]> {
  const result = new Map<string, Task[]>();
  const lines = body.split('\n');

  let currentSectionId = '';
  let currentTasks: Task[] = [];

  for (const line of lines) {
    // Match section headers: digits, uppercase letters, dots in any combination
    const sectionMatch = line.match(/^##\s+([\dA-Z.]+)\s+(.+)/);
    if (sectionMatch) {
      if (currentSectionId) {
        result.set(currentSectionId, currentTasks);
      }
      currentSectionId = sectionMatch[1];
      currentTasks = [];
      continue;
    }

    // Match: - [x] **Verb**: Description text — optional spec reference
    const checkboxMatch = line.match(/^-\s+\[([ xX])\]\s+\*\*(.+?)\*\*:?\s*(.*)/);
    if (checkboxMatch && currentSectionId) {
      const done = checkboxMatch[1].toLowerCase() === 'x';
      const verb = checkboxMatch[2].trim();
      const description = checkboxMatch[3]
        ?.replace(/\s*—\s*spec\/.*$/, '')
        ?.replace(/\s*—\s*[A-Za-z\/]+\.md.*$/, '')
        ?.replace(/`/g, '')
        ?.trim() || '';
      const name = description || verb;
      currentTasks.push({ name, done });
      continue;
    }

    // Fallback: Match plain checkbox without bold
    const plainCheckboxMatch = line.match(/^-\s+\[([ xX])\]\s+(.+)/);
    if (plainCheckboxMatch && currentSectionId) {
      const done = plainCheckboxMatch[1].toLowerCase() === 'x';
      const name = plainCheckboxMatch[2]
        ?.replace(/\s*—\s*.*$/, '')
        ?.replace(/`/g, '')
        ?.trim() || '';
      if (name) {
        currentTasks.push({ name, done });
      }
    }
  }

  if (currentSectionId) {
    result.set(currentSectionId, currentTasks);
  }

  return result;
}

// ============================================================================
// Status & Task Helpers
// ============================================================================

/** Normalize status values to handle both hyphen and underscore variants. */
export function normalizeStatus(status: string): string {
  return status.toLowerCase().replace(/_/g, '-');
}

/** Count done/total tasks across subsections. */
export function countTasks(subsections: Section[]): { done: number; total: number } {
  let done = 0;
  let total = 0;
  for (const subsection of subsections) {
    for (const task of subsection.tasks) {
      total++;
      if (task.done) done++;
    }
  }
  return { done, total };
}

// ============================================================================
// Section File Loading
// ============================================================================

/**
 * Load and parse a single section file into a RoadmapSection.
 */
export function loadSectionFile(filepath: string): RoadmapSection | null {
  if (!existsSync(filepath)) return null;

  const content = readFileSync(filepath, 'utf-8');
  if (!content.startsWith('---')) return null;

  const endIndex = content.indexOf('---', 3);
  if (endIndex === -1) return null;

  const frontmatterStr = content.slice(3, endIndex);
  const body = content.slice(endIndex + 3);

  const frontmatter = parseYamlFrontmatter(frontmatterStr);
  if (!frontmatter) return null;

  const tasksMap = extractTasksFromBody(body);

  const subsections: Section[] = (frontmatter.sections || []).map(s => ({
    name: `${s.id} ${s.title}`,
    tasks: tasksMap.get(s.id) || [],
  }));

  let doneCount = 0, totalCount = 0;
  for (const subsection of subsections) {
    for (const task of subsection.tasks) {
      totalCount++;
      if (task.done) doneCount++;
    }
  }

  const normalizedStatus = normalizeStatus(frontmatter.status);
  const status: 'complete' | 'partial' | 'not-started' =
    normalizedStatus === 'complete' ? 'complete' :
    normalizedStatus === 'in-progress' ? 'partial' : 'not-started';

  const filename = basename(filepath, '.md');

  return {
    num: frontmatter.section,
    slug: filename.toLowerCase(),
    name: frontmatter.title,
    status,
    note: totalCount > 0 ? `${doneCount}/${totalCount} tasks` : undefined,
    goal: frontmatter.goal,
    spec: Array.isArray(frontmatter.spec) ? frontmatter.spec.join(', ') : frontmatter.spec,
    subsections,
  };
}

/**
 * Load all section-*.md files from a directory into a sorted list.
 */
export function loadAllSections(dir: string): RoadmapSection[] {
  if (!existsSync(dir)) return [];

  const files = readdirSync(dir)
    .filter(f => f.startsWith('section-') && f.endsWith('.md'))
    .sort();

  const sections: RoadmapSection[] = [];
  for (const file of files) {
    const section = loadSectionFile(join(dir, file));
    if (section) sections.push(section);
  }
  return sections;
}

/** Look up a reroute by its URL key. */
export function findRerouteByKey(key: string): Reroute | undefined {
  return reroutes.find(r => r.key === key);
}
