import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';

const docs = defineCollection({
  loader: glob({ pattern: '**/*.{md,mdx}', base: './src/content/docs' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    section: z.enum(['getting-started']),
    order: z.number(),
  }),
});

const spec = defineCollection({
  loader: glob({ pattern: '[0-9][0-9]-*.md', base: '../docs/ori_lang/0.1-alpha/spec' }),
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number(),
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

export const collections = { docs, spec, 'compiler-design': compilerDesign };
