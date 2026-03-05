import { defineCollection, z } from 'astro:content';
import { glob } from 'astro/loaders';
import { proposalLoader } from './loaders/proposal-loader';
import { planSectionLoader } from './loaders/plan-section-loader';
import { reroutes, parallelPlans } from './lib/plan-data';

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
  loader: glob({ pattern: '{index,foreword,introduction,bibliography,grammar,operator-rules,[0-9][0-9]-*,annex-*}.md', base: '../docs/ori_lang/v2026/spec' }),
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

const roadmap = defineCollection({
  loader: glob({ pattern: 'section-*.md', base: '../plans/roadmap' }),
  schema: z.object({
    section: z.union([z.number(), z.string()]),
    title: z.string(),
    status: z.string(),
    tier: z.number(),
    goal: z.string(),
    spec: z.union([z.string(), z.array(z.string())]).optional(),
    priority_note: z.string().optional(),
    sections: z.array(z.object({
      id: z.string(),
      title: z.string(),
      status: z.string(),
    })),
  }),
});

const allPlans = [...reroutes, ...parallelPlans];

const planSections = defineCollection({
  loader: planSectionLoader({
    plans: allPlans.map(r => ({ key: r.key, base: `../plans/${r.dir}` })),
  }),
  schema: z.object({
    plan: z.string(),
    section: z.union([z.number(), z.string()]),
    title: z.string(),
    status: z.string(),
    goal: z.string().optional(),
    tier: z.number().optional(),
    spec: z.union([z.string(), z.array(z.string())]).optional(),
    inspired_by: z.array(z.string()).optional(),
    depends_on: z.array(z.string()).optional(),
    sections: z.array(z.object({
      id: z.string(),
      title: z.string(),
      status: z.string(),
    })),
  }),
});

const blog = defineCollection({
  loader: glob({ pattern: '*.md', base: '../blog' }),
  schema: z.object({
    title: z.string(),
    date: z.coerce.date(),
    description: z.string(),
  }),
});

const journeys = defineCollection({
  loader: glob({ pattern: '[0-9][0-9]-*-results.md', base: '../plans/code-journeys' }),
  schema: z.object({
    journey: z.number(),
    slug: z.string(),
    theme: z.string(),
    date: z.coerce.date(),
    status: z.enum(['PASS', 'FAIL_EVAL', 'FAIL_AOT', 'FAIL_BOTH']),
    expected: z.number(),
    eval_result: z.number(),
    aot_result: z.number(),
    difficulty: z.enum(['simple', 'moderate', 'complex']),
    prerequisites: z.array(z.string()).default([]),
    learning_objectives: z.array(z.string()).default([]),
    features: z.array(z.string()),
    feature_description: z.string(),
    score: z.number(),
    score_breakdown: z.object({
      instruction_efficiency: z.number(),
      arc_correctness: z.number(),
      attributes_safety: z.number(),
      control_flow: z.number(),
      ir_quality: z.number(),
      binary_quality: z.number(),
    }),
    overflow_check: z.enum(['PASS', 'FAIL']),
    bugs_found: z.array(z.object({
      id: z.string(),
      severity: z.string(),
      description: z.string(),
      status: z.enum(['OPEN', 'FIXED']),
      found_in: z.string(),
      fixed_in: z.string().optional(),
    })).default([]),
    related_journeys: z.array(z.object({
      journey: z.number(),
      relationship: z.string(),
    })).default([]),
  }),
});

const proposals = defineCollection({
  loader: proposalLoader({ base: '../docs/ori_lang/proposals' }),
  schema: z.object({
    title: z.string(),
    status: z.enum(['approved', 'draft', 'rejected']),
    author: z.string().optional(),
    created: z.string().optional(),
    approved: z.string().optional(),
    rejected: z.string().optional(),
    summary: z.string().optional(),
  }),
});

export const collections = { guide, spec, 'compiler-design': compilerDesign, formatter, lsp, roadmap, 'plan-sections': planSections, proposals, blog, journeys };
