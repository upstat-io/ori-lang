/**
 * Remark plugin to transform markdown links (.md) to proper absolute URLs.
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
    // Debug: log what's available
    console.log('[remark-md-links] file keys:', Object.keys(file || {}));
    console.log('[remark-md-links] file.data keys:', Object.keys(file?.data || {}));
    console.log('[remark-md-links] file.history:', file?.history);
    console.log('[remark-md-links] file.path:', file?.path);

    // Try to get file path from various sources
    const filePath = file?.history?.[0] || file?.path || file?.data?.astro?.filePath;

    if (!filePath) {
      // No file path available - can't do relative resolution
      // Fall back to just stripping .md extension
      visit(tree, 'link', (node) => {
        if (node.url && node.url.includes('.md') && !node.url.startsWith('http')) {
          node.url = node.url.replace(/\.md(#|$)/, '$1');
        }
      });
      return;
    }

    // Find which collection this file belongs to
    const mapping = COLLECTION_MAPPINGS.find(m => filePath.includes(m.sourceBase));
    if (!mapping) {
      // Not in a known collection - just strip .md
      visit(tree, 'link', (node) => {
        if (node.url && node.url.includes('.md') && !node.url.startsWith('http')) {
          node.url = node.url.replace(/\.md(#|$)/, '$1');
        }
      });
      return;
    }

    // Get the file's directory relative to the collection base
    const sourceBaseIndex = filePath.indexOf(mapping.sourceBase);
    const relativePath = filePath.slice(sourceBaseIndex + mapping.sourceBase.length + 1);
    const fileDir = dirname(relativePath);

    visit(tree, 'link', (node) => {
      const url = node.url;

      // Skip external URLs, absolute paths, and anchor-only links
      if (!url || url.startsWith('http://') || url.startsWith('https://') ||
          url.startsWith('/') || url.startsWith('#')) {
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
      node.url = `${mapping.urlBase}/${resolvedPath}${anchor ? '#' + anchor : ''}`;
    });
  };
}

export default remarkMdLinks;
