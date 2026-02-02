/**
 * Remark plugin to transform markdown links (.md) to proper absolute URLs.
 *
 * Problem: When a page URL is /docs/compiler-design/03-lexer (no trailing slash),
 * browsers treat "03-lexer" as a file, so relative links resolve incorrectly:
 * - "token-design" -> /docs/compiler-design/token-design (WRONG)
 * - Should be: /docs/compiler-design/03-lexer/token-design
 *
 * Solution: Convert relative .md links to absolute paths at build time.
 *
 * Transforms (example from docs/compiler/design/03-lexer/index.md):
 * - [Token Design](token-design.md) -> [Token Design](/docs/compiler-design/03-lexer/token-design)
 * - [Pipeline](../01-architecture/pipeline.md) -> [Pipeline](/docs/compiler-design/01-architecture/pipeline)
 *
 * Does NOT transform:
 * - External URLs (http://, https://)
 * - Already absolute paths starting with /
 * - Anchor-only links (#section)
 */

import { visit } from 'unist-util-visit';
import { dirname, join, normalize } from 'path';

// Map source directories to URL base paths
const COLLECTION_MAPPINGS = [
  { sourceBase: 'docs/ori_lang/0.1-alpha/spec', urlBase: '/docs/spec' },
  { sourceBase: 'docs/compiler/design', urlBase: '/docs/compiler-design' },
  { sourceBase: 'docs/tooling/formatter/design', urlBase: '/docs/formatter' },
  { sourceBase: 'docs/tooling/lsp/design', urlBase: '/docs/lsp' },
  { sourceBase: 'docs/guide', urlBase: '/guide' },
];

export function remarkMdLinks() {
  return (tree, file) => {
    // Get the source file path
    const filePath = file.history?.[0] || file.path;
    console.log('[remark-md-links] Processing file:', filePath);
    if (!filePath) {
      console.log('[remark-md-links] No file path found, file object:', Object.keys(file));
      return;
    }

    // Find which collection this file belongs to
    const mapping = COLLECTION_MAPPINGS.find(m => filePath.includes(m.sourceBase));
    if (!mapping) return;

    // Get the file's directory relative to the collection base
    const sourceBaseIndex = filePath.indexOf(mapping.sourceBase);
    const relativePath = filePath.slice(sourceBaseIndex + mapping.sourceBase.length + 1);
    const fileDir = dirname(relativePath);

    visit(tree, 'link', (node) => {
      const url = node.url;

      // Skip external URLs
      if (url.startsWith('http://') || url.startsWith('https://')) {
        return;
      }

      // Skip already absolute paths
      if (url.startsWith('/')) {
        return;
      }

      // Skip anchor-only links
      if (url.startsWith('#')) {
        return;
      }

      // Only transform .md links
      if (!url.includes('.md')) {
        return;
      }

      // Split URL into path and anchor
      const [pathPart, anchor] = url.split('#');

      // Remove .md extension
      const cleanPath = pathPart.replace(/\.md$/, '');

      // Resolve the relative path
      const resolvedPath = fileDir === '.'
        ? cleanPath
        : normalize(join(fileDir, cleanPath));

      // Build absolute URL
      const absoluteUrl = `${mapping.urlBase}/${resolvedPath}${anchor ? '#' + anchor : ''}`;

      // Normalize any remaining ../ or ./
      node.url = absoluteUrl.replace(/\/\.\//g, '/');
      console.log(`[remark-md-links] ${url} -> ${node.url}`);
    });
  };
}

export default remarkMdLinks;
