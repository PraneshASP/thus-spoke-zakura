import { cn } from '@/lib/cn';

/**
 * A summary tile. Shared so the four detail pages cannot drift — they each
 * had a private copy, and one had already lost the `tone` prop.
 */
export function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: 'accent' | 'muted';
}) {
  return (
    <div className="xp-window rounded-xs px-4 py-3">
      <span className="text-ink-muted block text-[11px] font-bold tracking-[0.12em] uppercase">
        {label}
      </span>
      <strong
        className={cn(
          'mt-1 block text-[15px] font-bold tabular-nums',
          tone === 'accent' && 'text-accent-strong',
          tone === 'muted' && 'text-ink-muted',
        )}
      >
        {value}
      </strong>
    </div>
  );
}
