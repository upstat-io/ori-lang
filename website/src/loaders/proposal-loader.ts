import type { Loader } from 'astro/loaders';
import { readFileSync, readdirSync, existsSync } from 'fs';
import { join, resolve } from 'path';

interface ProposalLoaderOptions {
  base: string;
}

interface ParsedProposal {
  title: string;
  status: 'approved' | 'draft' | 'rejected';
  author?: string;
  created?: string;
  approved?: string;
  rejected?: string;
  summary?: string;
}

function parseProposalContent(content: string, filename: string, status: 'approved' | 'draft' | 'rejected'): ParsedProposal {
  const lines = content.split('\n');

  // Extract title from first H1
  const titleMatch = lines[0]?.match(/^#\s+(?:Proposal:\s*)?(.+)/);
  const title = titleMatch ? titleMatch[1].trim() : filename.replace(/-proposal\.md$/, '').replace(/-/g, ' ');

  // Extract metadata from **Key:** Value format in first 20 lines
  const getField = (key: string): string | undefined => {
    const regex = new RegExp(`\\*\\*${key}:\\*\\*\\s*(.+)`, 'i');
    for (const line of lines.slice(0, 20)) {
      const match = line.match(regex);
      if (match) return match[1].trim();
    }
    return undefined;
  };

  // Extract summary - first paragraph after "## Summary"
  let summary: string | undefined;
  const summaryIdx = lines.findIndex(l => l.match(/^##\s+Summary/i));
  if (summaryIdx !== -1) {
    const summaryLines: string[] = [];
    for (let i = summaryIdx + 1; i < lines.length; i++) {
      const line = lines[i];
      if (line.startsWith('#') || line.startsWith('---')) break;
      if (line.trim()) summaryLines.push(line.trim());
      else if (summaryLines.length > 0) break;
    }
    summary = summaryLines.join(' ').slice(0, 200);
    if (summary.length === 200) summary += '...';
  }

  return {
    title,
    status,
    author: getField('Author'),
    created: getField('Created'),
    approved: getField('Approved'),
    rejected: getField('Rejected'),
    summary,
  };
}

export function proposalLoader(options: ProposalLoaderOptions): Loader {
  const subdirs: Array<{ dir: string; status: 'approved' | 'draft' | 'rejected' }> = [
    { dir: 'approved', status: 'approved' },
    { dir: 'drafts', status: 'draft' },
    { dir: 'rejected', status: 'rejected' },
  ];

  return {
    name: 'proposal-loader',

    load: async ({ store, parseData, renderMarkdown, generateDigest, logger }) => {
      const baseDir = resolve(process.cwd(), options.base);
      logger.info(`Loading proposals from ${baseDir}`);

      store.clear();

      for (const { dir, status } of subdirs) {
        const fullDir = join(baseDir, dir);
        if (!existsSync(fullDir)) {
          logger.warn(`Proposals directory not found: ${fullDir}`);
          continue;
        }

        const files = readdirSync(fullDir).filter(f => f.endsWith('.md'));

        for (const file of files) {
          const filePath = join(fullDir, file);
          const content = readFileSync(filePath, 'utf-8');
          const slug = file.replace(/\.md$/, '');
          const id = `${status}/${slug}`;

          const parsed = parseProposalContent(content, file, status);

          const data = await parseData({
            id,
            data: {
              title: parsed.title,
              status: parsed.status,
              author: parsed.author,
              created: parsed.created,
              approved: parsed.approved,
              rejected: parsed.rejected,
              summary: parsed.summary,
            },
          });

          const rendered = await renderMarkdown(content, {
            fileURL: new URL(`file://${filePath}`),
          });
          const digest = generateDigest(data);

          store.set({
            id,
            data,
            body: content,
            rendered,
            digest,
            filePath: `docs/ori_lang/proposals/${dir}/${file}`,
          });
        }
      }
    },
  };
}
