/**
 * Build script: reads tutorial lesson directories and produces
 * public/tutorial-manifest.json for the Svelte tutorial shell.
 *
 * Run via: node scripts/build-tutorial.mjs
 * Added to package.json prebuild step.
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync, readdirSync, statSync } from 'fs';
import { join, resolve } from 'path';
import { marked } from 'marked';
import matter from 'gray-matter';

const CONTENT_DIR = resolve('src/content/tutorial');
const OUTPUT_FILE = resolve('public/tutorial-manifest.json');

function readFileOr(path, fallback) {
  try {
    return readFileSync(path, 'utf-8');
  } catch {
    return fallback;
  }
}

function getSortedDirs(parentDir) {
  if (!existsSync(parentDir)) return [];
  return readdirSync(parentDir)
    .filter(name => {
      const full = join(parentDir, name);
      return statSync(full).isDirectory() && !name.startsWith('.');
    })
    .sort();
}

function buildManifest() {
  if (!existsSync(CONTENT_DIR)) {
    console.warn(`[build-tutorial] No tutorial content at ${CONTENT_DIR}, writing empty manifest.`);
    writeFileSync(OUTPUT_FILE, JSON.stringify({ chapters: [], lessons: {} }, null, 2));
    return;
  }

  const chapters = [];
  const lessons = {};

  const chapterDirs = getSortedDirs(CONTENT_DIR);

  for (const chapterDir of chapterDirs) {
    const chapterPath = join(CONTENT_DIR, chapterDir);
    const metaPath = join(chapterPath, '_meta.json');
    const chapterMeta = existsSync(metaPath)
      ? JSON.parse(readFileSync(metaPath, 'utf-8'))
      : { title: chapterDir };

    const chapter = {
      slug: chapterDir,
      title: chapterMeta.title,
      lessons: [],
    };

    const lessonDirs = getSortedDirs(chapterPath);

    for (const lessonDir of lessonDirs) {
      const lessonPath = join(chapterPath, lessonDir);
      const slug = `${chapterDir}/${lessonDir}`;

      // Read content.md (frontmatter + prose)
      const contentMd = readFileOr(join(lessonPath, 'content.md'), '');
      const { data: frontmatter, content: prose } = matter(contentMd);
      const contentHtml = marked.parse(prose);

      // Read code files
      const starterCode = readFileOr(join(lessonPath, 'starter.ori'), '');
      const solutionCode = readFileOr(join(lessonPath, 'solution.ori'), '');

      // Read validation config
      const metaJson = readFileOr(join(lessonPath, 'meta.json'), '{}');
      const meta = JSON.parse(metaJson);

      const order = frontmatter.order ?? (parseInt(lessonDir.split('-')[0], 10) || 0);
      const title = frontmatter.title ?? lessonDir;
      const description = frontmatter.description ?? '';

      const lesson = {
        slug,
        title,
        chapter: chapterMeta.title,
        order,
        description,
        contentHtml,
        starterCode,
        solutionCode,
        validation: {
          expectedOutput: meta.expected_output ?? '',
          outputMatch: meta.output_match ?? 'none',
          hints: meta.hints ?? [],
        },
      };

      lessons[slug] = lesson;
      chapter.lessons.push({ slug, title, order });
    }

    // Sort lessons by order within the chapter
    chapter.lessons.sort((a, b) => a.order - b.order);

    if (chapter.lessons.length > 0) {
      chapters.push(chapter);
    }
  }

  if (!existsSync('public')) {
    mkdirSync('public', { recursive: true });
  }

  writeFileSync(OUTPUT_FILE, JSON.stringify({ chapters, lessons }, null, 2));

  const totalLessons = Object.keys(lessons).length;
  console.log(`[build-tutorial] Built manifest: ${chapters.length} chapters, ${totalLessons} lessons → ${OUTPUT_FILE}`);
}

buildManifest();
