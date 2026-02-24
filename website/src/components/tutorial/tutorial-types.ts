export interface TutorialManifest {
  chapters: TutorialChapter[];
  lessons: Record<string, TutorialLesson>;
}

export interface TutorialChapter {
  slug: string;
  title: string;
  lessons: TutorialLessonRef[];
}

export interface TutorialLessonRef {
  slug: string;
  title: string;
  order: number;
}

export interface TutorialLesson {
  slug: string;
  title: string;
  chapter: string;
  order: number;
  description: string;
  contentHtml: string;
  starterCode: string;
  solutionCode: string;
  validation: LessonValidation;
}

export interface LessonValidation {
  expectedOutput: string;
  outputMatch: 'exact' | 'contains' | 'regex' | 'none';
  hints: string[];
}
