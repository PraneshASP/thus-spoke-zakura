import { Monitor, Moon, Sun } from 'lucide-react';
import { useTheme } from '@/lib/useTheme';
import type { ThemePreference } from '@/lib/theme';
import { cn } from '@/lib/cn';

const OPTIONS: ReadonlyArray<{ value: ThemePreference; label: string; Icon: typeof Sun }> = [
  { value: 'light', label: 'Light', Icon: Sun },
  { value: 'system', label: 'System', Icon: Monitor },
  { value: 'dark', label: 'Dark', Icon: Moon },
];

/**
 * Three explicit states rather than a two-way switch: "system" has to be
 * reachable, otherwise choosing light once permanently opts the user out of
 * following their OS.
 */
export function ThemeToggle() {
  const [preference, setPreference] = useTheme();

  return (
    <div
      role="radiogroup"
      aria-label="Colour theme"
      className="xp-sunken bg-sunken flex gap-0.5 rounded-xs p-0.5"
    >
      {OPTIONS.map(({ value, label, Icon }) => {
        const active = preference === value;
        return (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={active}
            aria-label={label}
            title={label}
            onClick={() => setPreference(value)}
            className={cn(
              'grid flex-1 cursor-pointer place-items-center rounded-xs py-1.5 transition-colors',
              active ? 'xp-raised bg-raised text-accent-strong' : 'text-ink-muted hover:text-ink',
            )}
          >
            <Icon className="size-3.5" />
          </button>
        );
      })}
    </div>
  );
}
