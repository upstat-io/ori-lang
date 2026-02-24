const STORAGE_KEY = 'ori-tutorial';

interface TutorialProgress {
  completed: string[];
  savedCode: Record<string, string>;
  lastSlug?: string;
}

function getStorage(): TutorialProgress {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw);
  } catch {
    // Ignore parse errors or missing localStorage
  }
  return { completed: [], savedCode: {} };
}

function setStorage(progress: TutorialProgress): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(progress));
  } catch {
    // Ignore quota errors
  }
}

export function loadProgress(): TutorialProgress {
  return getStorage();
}

export function isCompleted(slug: string): boolean {
  return getStorage().completed.includes(slug);
}

export function markCompleted(slug: string): void {
  const progress = getStorage();
  if (!progress.completed.includes(slug)) {
    progress.completed.push(slug);
    setStorage(progress);
  }
}

export function saveCode(slug: string, code: string): void {
  const progress = getStorage();
  progress.savedCode[slug] = code;
  setStorage(progress);
}

export function getSavedCode(slug: string): string | null {
  return getStorage().savedCode[slug] ?? null;
}

export function savePosition(slug: string): void {
  const progress = getStorage();
  progress.lastSlug = slug;
  setStorage(progress);
}

export function getLastPosition(): string | undefined {
  return getStorage().lastSlug;
}

export function resetProgress(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Ignore
  }
}
