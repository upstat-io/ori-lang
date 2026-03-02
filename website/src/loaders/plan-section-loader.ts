import type { Loader } from 'astro/loaders';
import { readFileSync, readdirSync, existsSync } from 'fs';
import { join, resolve } from 'path';
import { parseYamlFrontmatter } from '../lib/plan-data';

interface PlanEntry {
  key: string;   // URL-friendly key (e.g., 'value-semantics-optimization')
  base: string;  // relative path from website root (e.g., '../plans/value-semantics-optimization')
}

interface PlanSectionLoaderOptions {
  plans: PlanEntry[];
}

export function planSectionLoader(options: PlanSectionLoaderOptions): Loader {
  return {
    name: 'plan-section-loader',

    load: async ({ store, parseData, renderMarkdown, generateDigest, logger }) => {
      store.clear();

      for (const plan of options.plans) {
        const baseDir = resolve(process.cwd(), plan.base);

        if (!existsSync(baseDir)) {
          logger.warn(`Plan directory not found: ${baseDir}`);
          continue;
        }

        const files = readdirSync(baseDir)
          .filter(f => f.startsWith('section-') && f.endsWith('.md'))
          .sort();

        logger.info(`Loading ${files.length} sections from ${plan.key}`);

        for (const file of files) {
          const filePath = join(baseDir, file);
          const content = readFileSync(filePath, 'utf-8');

          // Extract frontmatter
          if (!content.startsWith('---')) {
            logger.warn(`No frontmatter in ${filePath}`);
            continue;
          }

          const endIndex = content.indexOf('---', 3);
          if (endIndex === -1) {
            logger.warn(`Unclosed frontmatter in ${filePath}`);
            continue;
          }

          const frontmatterStr = content.slice(3, endIndex);
          const body = content.slice(endIndex + 3);

          const frontmatter = parseYamlFrontmatter(frontmatterStr);
          if (!frontmatter) {
            logger.warn(`Failed to parse frontmatter in ${filePath}`);
            continue;
          }

          const slug = file.replace(/\.md$/, '');
          const id = `${plan.key}/${slug}`;

          const data = await parseData({
            id,
            data: {
              plan: plan.key,
              section: frontmatter.section,
              title: frontmatter.title,
              status: frontmatter.status,
              goal: frontmatter.goal,
              tier: frontmatter.tier,
              spec: frontmatter.spec,
              inspired_by: frontmatter.inspired_by,
              depends_on: frontmatter.depends_on,
              sections: frontmatter.sections || [],
            },
          });

          const rendered = await renderMarkdown(body, {
            fileURL: new URL(`file://${filePath}`),
          });
          const digest = generateDigest(data);

          store.set({
            id,
            data,
            body,
            rendered,
            digest,
            filePath: filePath.replace(resolve(process.cwd(), '..') + '/', ''),
          });
        }
      }
    },
  };
}
