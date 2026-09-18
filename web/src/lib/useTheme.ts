import { useCallback, useSyncExternalStore } from 'react';
import { applyTheme, readStoredTheme, THEME_STORAGE_KEY, type ThemePreference } from './theme';

const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

function subscribe(onChange: () => void): () => void {
  listeners.add(onChange);
  // Keep other tabs in step.
  window.addEventListener('storage', onChange);
  return () => {
    listeners.delete(onChange);
    window.removeEventListener('storage', onChange);
  };
}

/**
 * The theme preference, read through useSyncExternalStore for the same reason
 * as the motion preference: it lives outside React (the DOM attribute and
 * localStorage), so it is subscribed to rather than mirrored into state.
 */
export function useTheme(): [ThemePreference, (next: ThemePreference) => void] {
  const preference = useSyncExternalStore(subscribe, readStoredTheme, () => 'system' as const);

  const setPreference = useCallback((next: ThemePreference) => {
    try {
      localStorage.setItem(THEME_STORAGE_KEY, next);
    } catch {
      // Preference simply will not persist; applying it still works.
    }
    applyTheme(next);
    emit();
  }, []);

  return [preference, setPreference];
}
