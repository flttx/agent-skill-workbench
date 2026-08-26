export type ThemeId =
  | "pillars"
  | "black-hole"
  | "starweaver"
  | "cosmic-cliffs";

export interface ThemeDefinition {
  id: ThemeId;
  name: string;
  description: string;
  image: string;
}

export const THEME_STORAGE_KEY = "agent-workbench-theme";
export const DEFAULT_THEME_ID: ThemeId = "pillars";

export const THEMES: readonly ThemeDefinition[] = [
  {
    id: "pillars",
    name: "创生之柱",
    description: "秘境苍金",
    image: "/创生之柱.jpg",
  },
  {
    id: "black-hole",
    name: "黑洞引力",
    description: "深邃赤黑",
    image: "/黑洞引力.jpg",
  },
  {
    id: "starweaver",
    name: "恒星编织者",
    description: "幽影苍桃",
    image: "/恒星编织者.jpg",
  },
  {
    id: "cosmic-cliffs",
    name: "宇宙山脉",
    description: "绯红尘埃",
    image: "/宇宙山脉.jpg",
  },
];

const themeIds = new Set<ThemeId>(THEMES.map((theme) => theme.id));

export function isThemeId(value: string | null): value is ThemeId {
  return value !== null && themeIds.has(value as ThemeId);
}

export function getStoredTheme(): ThemeId {
  try {
    const storedTheme = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeId(storedTheme) ? storedTheme : DEFAULT_THEME_ID;
  } catch {
    return DEFAULT_THEME_ID;
  }
}

export function persistTheme(themeId: ThemeId) {
  try {
    localStorage.setItem(THEME_STORAGE_KEY, themeId);
  } catch {
    // Theme switching still works for the current session when storage is unavailable.
  }
}

export function applyTheme(themeId: ThemeId) {
  document.documentElement.dataset.theme = themeId;
}
