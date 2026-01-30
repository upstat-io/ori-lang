import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const guide = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/guide' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
    section: z.string().optional(),
    part: z.string().optional(),
  }),
});

const spec = defineCollection({
  loader: glob({ pattern: '{index,[0-9][0-9]-*}.md', base: '../docs/ori_lang/0.1-alpha/spec' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
    section: z.string().optional(),
  }),
});

const compilerDesign = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/compiler/design' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
    section: z.string().optional(),
  }),
});

const formatter = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/tooling/formatter/design' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
    section: z.string().optional(),
  }),
});

const lsp = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/tooling/lsp/design' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
    section: z.string().optional(),
  }),
});

export const collections = { guide, spec, 'compiler-design': compilerDesign, formatter, lsp };
