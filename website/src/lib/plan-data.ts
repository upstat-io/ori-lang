import { readFileSync, readdirSync, existsSync } from 'fs';
import { join, basename, resolve } from 'path';

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
// Reroute Plan Registry (loaded from plan index.md frontmatter)
// ============================================================================

/**
 * Scan plan directories for index.md files with `reroute: true` frontmatter.
 * Each reroute plan's index.md is the single source of truth for its metadata.
 *
 * @param plansBase - path to plans directory, relative to process.cwd()
 */
export function loadReroutes(plansBase: string = '../plans'): Reroute[] {
  const plansDir = resolve(process.cwd(), plansBase);
  const results: Reroute[] = [];

  if (!existsSync(plansDir)) return results;

  const dirs = readdirSync(plansDir, { withFileTypes: true })
    .filter(d => d.isDirectory() && !d.name.startsWith('_'));

  for (const d of dirs) {
    const indexPath = join(plansDir, d.name, 'index.md');
    if (!existsSync(indexPath)) continue;

    const content = readFileSync(indexPath, 'utf-8');
    if (!content.startsWith('---')) continue;

    const endIndex = content.indexOf('---', 3);
    if (endIndex === -1) continue;

    const yamlStr = content.slice(3, endIndex);
    const parsed = parseYamlFrontmatter(yamlStr) as unknown as Record<string, unknown>;
    if (!parsed || parsed.reroute !== true) continue;

    const dir = d.name;
    const key = dir.replace(/_/g, '-');

    results.push({
      name: parsed.name as string,
      fullName: parsed.full_name as string,
      status: parsed.status as 'active' | 'queued' | 'resolved',
      key,
      dir,
    });
  }

  // Sort: active first, then queued, then resolved
  const statusOrder: Record<string, number> = { active: 0, queued: 1, resolved: 2 };
  results.sort((a, b) => (statusOrder[a.status] ?? 3) - (statusOrder[b.status] ?? 3));

  return results;
}

export const reroutes: Reroute[] = loadReroutes();

// ============================================================================
// Parallel Plan Registry (loaded from plan index.md frontmatter)
// ============================================================================

/**
 * Scan plan directories for index.md files with `parallel: true` frontmatter.
 * Same shape as reroutes — just a different discriminator field.
 */
export function loadParallelPlans(plansBase: string = '../plans'): Reroute[] {
  const plansDir = resolve(process.cwd(), plansBase);
  const results: Reroute[] = [];

  if (!existsSync(plansDir)) return results;

  const dirs = readdirSync(plansDir, { withFileTypes: true })
    .filter(d => d.isDirectory() && !d.name.startsWith('_'));

  for (const d of dirs) {
    const indexPath = join(plansDir, d.name, 'index.md');
    if (!existsSync(indexPath)) continue;

    const content = readFileSync(indexPath, 'utf-8');
    if (!content.startsWith('---')) continue;

    const endIndex = content.indexOf('---', 3);
    if (endIndex === -1) continue;

    const yamlStr = content.slice(3, endIndex);
    const parsed = parseYamlFrontmatter(yamlStr) as unknown as Record<string, unknown>;
    if (!parsed || parsed.parallel !== true) continue;

    const dir = d.name;
    const key = dir.replace(/_/g, '-');

    results.push({
      name: parsed.name as string,
      fullName: parsed.full_name as string,
      status: parsed.status as 'active' | 'queued' | 'resolved',
      key,
      dir,
    });
  }

  const statusOrder: Record<string, number> = { active: 0, queued: 1, resolved: 2 };
  results.sort((a, b) => (statusOrder[a.status] ?? 3) - (statusOrder[b.status] ?? 3));

  return results;
}

export const parallelPlans: Reroute[] = loadParallelPlans();

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

/** Count done/total tasks across subsections. If the section is complete, treat all tasks as done. */
export function countTasks(subsections: Section[], sectionStatus?: 'complete' | 'partial' | 'not-started'): { done: number; total: number } {
  let done = 0;
  let total = 0;
  for (const subsection of subsections) {
    for (const task of subsection.tasks) {
      total++;
      if (task.done) done++;
    }
  }
  if (sectionStatus === 'complete' && total > 0) {
    done = total;
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

/** Look up any plan (reroute or parallel) by its URL key. */
export function findPlanByKey(key: string): Reroute | undefined {
  return reroutes.find(r => r.key === key) ?? parallelPlans.find(p => p.key === key);
}

// ============================================================================
// Mission Extraction
// ============================================================================

/**
 * Extract the `## Mission` section from a plan's 00-overview.md.
 * Returns the text content (without the heading), or null if not found.
 */
export function loadMission(planDir: string): string | null {
  const overviewPath = join(planDir, '00-overview.md');
  if (!existsSync(overviewPath)) return null;

  let content = readFileSync(overviewPath, 'utf-8');

  // Skip frontmatter if present
  if (content.startsWith('---')) {
    const endIndex = content.indexOf('---', 3);
    if (endIndex !== -1) {
      content = content.slice(endIndex + 3);
    }
  }

  // Find ## Mission heading
  const missionMatch = content.match(/^## Mission\s*\n([\s\S]*?)(?=\n## |\n---|\s*$)/m);
  if (!missionMatch) return null;

  const text = missionMatch[1].trim();
  return text || null;
}

/**
 * Convert basic inline markdown to HTML for `set:html` rendering.
 * Handles **bold**, *italic*, `code`, and paragraph breaks.
 */
export function inlineMarkdownToHtml(md: string): string {
  const paragraphs = md.split(/\n\n+/);
  return paragraphs
    .map(p => {
      let html = p.trim();
      // Code spans first (to avoid nested transforms)
      html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
      // Bold
      html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
      // Italic
      html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');
      // Wrap single newlines as line continuation
      html = html.replace(/\n/g, ' ');
      return `<p>${html}</p>`;
    })
    .join('\n');
}
