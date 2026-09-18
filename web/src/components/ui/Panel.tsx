import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

/**
 * A titled window. Every list and detail surface uses this so caption weight,
 * padding and the meta slot stay identical across the app.
 */
export function Panel({
  title,
  eyebrow,
  meta,
  actions,
  children,
  className,
}: {
  title: string;
  eyebrow?: string;
  meta?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn('xp-window overflow-hidden rounded-xs', className)}>
      <header className="xp-caption flex items-center justify-between gap-3 px-4 py-2.5">
        <div className="flex min-w-0 items-baseline gap-2">
          {eyebrow && (
            <span className="text-ink-muted shrink-0 font-mono text-[11px] font-bold tracking-[0.13em]">
              {eyebrow}
            </span>
          )}
          <h2 className="truncate text-[13px] font-bold tracking-[0.01em]">{title}</h2>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {meta}
          {actions}
        </div>
      </header>
      {children}
    </section>
  );
}

/**
 * A label/value pair inside a Panel. Label styling is deliberately identical
 * to DataTable's column headers so detail views and list views read as the
 * same system.
 */
export function DataRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="border-line grid gap-1.5 border-b px-5 py-3.5 last:border-b-0 sm:grid-cols-[210px_1fr] sm:gap-5">
      <dt className="text-ink px-0 py-0 text-[11px] font-bold tracking-[0.12em] uppercase">
        {label}
      </dt>
      <dd className="text-ink min-w-0 text-[12px] break-all">{children}</dd>
    </div>
  );
}

/** A footnote inside a Panel, on the panel's own tinted footer. */
export function PanelNote({ children }: { children: ReactNode }) {
  return (
    <p className="text-ink-muted border-line bg-raised border-t px-5 py-4 text-[12px] leading-relaxed">
      {children}
    </p>
  );
}
