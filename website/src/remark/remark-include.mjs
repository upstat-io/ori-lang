/**
 * Remark plugin to handle mdbook-style {{#include path}} directives.
 *
 * Supports:
 * - {{#include path/to/file}} - Include entire file
 * - {{#include path/to/file:start:end}} - Include lines start to end (1-indexed)
 * - {{#include path/to/file::end}} - Include from beginning to line end
 * - {{#include path/to/file:start:}} - Include from line start to end
 *
 * Paths are resolved relative to the markdown file containing the directive.
 */

import { visit } from 'unist-util-visit';
import { readFileSync, existsSync } from 'fs';
import { dirname, resolve } from 'path';

const INCLUDE_REGEX = /\{\{#include\s+([^}]+)\}\}/g;

export function remarkInclude() {
  return (tree, file) => {
    const filePath = file.history[0];
    const fileDir = filePath ? dirname(filePath) : process.cwd();

    visit(tree, 'code', (node) => {
      if (!node.value) return;

      const match = node.value.match(INCLUDE_REGEX);
      if (!match) return;

      // Replace all include directives in the code block
      let newValue = node.value;

      for (const includeMatch of node.value.matchAll(INCLUDE_REGEX)) {
        const fullMatch = includeMatch[0];
        const includePath = includeMatch[1].trim();

        // Parse path and optional line range
        const [pathPart, startLine, endLine] = parseIncludePath(includePath);

        // Resolve the path relative to the markdown file
        const absolutePath = resolve(fileDir, pathPart);

        if (!existsSync(absolutePath)) {
          console.warn(`[remark-include] File not found: ${absolutePath} (referenced from ${filePath})`);
          newValue = newValue.replace(fullMatch, `// File not found: ${pathPart}`);
          continue;
        }

        try {
          let content = readFileSync(absolutePath, 'utf-8');

          // Apply line range if specified
          if (startLine !== undefined || endLine !== undefined) {
            const lines = content.split('\n');
            const start = startLine ? startLine - 1 : 0;
            const end = endLine ? endLine : lines.length;
            content = lines.slice(start, end).join('\n');
          }

          newValue = newValue.replace(fullMatch, content);
        } catch (err) {
          console.warn(`[remark-include] Error reading file: ${absolutePath}`, err.message);
          newValue = newValue.replace(fullMatch, `// Error reading: ${pathPart}`);
        }
      }

      node.value = newValue;
    });
  };
}

/**
 * Parse include path with optional line range.
 * Examples:
 * - "path/to/file" -> ["path/to/file", undefined, undefined]
 * - "path/to/file:10:20" -> ["path/to/file", 10, 20]
 * - "path/to/file::20" -> ["path/to/file", undefined, 20]
 * - "path/to/file:10:" -> ["path/to/file", 10, undefined]
 */
function parseIncludePath(includePath) {
  // Check for line range syntax (colon-separated)
  const colonMatch = includePath.match(/^(.+?):(\d*):(\d*)$/);
  if (colonMatch) {
    const path = colonMatch[1];
    const start = colonMatch[2] ? parseInt(colonMatch[2], 10) : undefined;
    const end = colonMatch[3] ? parseInt(colonMatch[3], 10) : undefined;
    return [path, start, end];
  }

  return [includePath, undefined, undefined];
}

export default remarkInclude;
