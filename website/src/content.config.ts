import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const docsSchema = z.object({
  title: z.string(),
  description: z.string().optional(),
  order: z.number(),
  section: z.string().optional(),
  // Sidebar metadata — only on root index.md files
  sidebar_title: z.string().optional(),
  sidebar_order: z.number().optional(),
  sidebar_path: z.string().optional(),
});

const guide = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/guide' }),
  schema: docsSchema.extend({ part: z.string().optional() }),
});

const spec = defineCollection({
  loader: glob({ pattern: '{index,[0-9][0-9]-*}.md', base: '../docs/ori_lang/0.1-alpha/spec' }),
  schema: docsSchema,
});

const compilerDesign = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/compiler/design' }),
  schema: docsSchema,
});

const formatter = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/tooling/formatter/design' }),
  schema: docsSchema,
});

const lsp = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/tooling/lsp/design' }),
  schema: docsSchema,
});

export const collections = { guide, spec, 'compiler-design': compilerDesign, formatter, lsp };
