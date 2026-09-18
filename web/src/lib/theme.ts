export type ThemePreference = 'light' | 'dark' | 'system';

export const THEME_STORAGE_KEY = 'tsz-theme';

export function isThemePreference(value: unknown): value is ThemePreference {
  return value === 'light' || value === 'dark' || value === 'system';
}

/**
 * Applies a preference to the document.
 *
 * "system" removes the attribute entirely rather than resolving it here, so the
 * CSS media query stays the single source of truth and the theme keeps
 * following the OS if it changes while the tab is open.
 */
export function applyTheme(preference: ThemePreference): void {
  const root = document.documentElement;
  if (preference === 'system') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', preference);
}

export function readStoredTheme(): ThemePreference {
  try {
    const stored = localStorage.getItem(THEME_STORAGE_KEY);
    return isThemePreference(stored) ? stored : 'system';
  } catch {
    // Private browsing or a blocked storage partition: fall back to system.
    return 'system';
  }
}
