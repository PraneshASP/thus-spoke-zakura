import { cva, type VariantProps } from 'class-variance-authority';
import type { ReactNode } from 'react';
import { cn } from '@/lib/cn';

const badge = cva(
  'inline-flex items-center justify-center rounded-xs border px-2 py-0.5 text-[11px] font-bold tracking-[0.08em] uppercase',
  {
    variants: {
      tone: {
        neutral: 'border-line bg-raised text-ink-muted',
        accent: 'border-accent-line bg-accent-soft text-accent-strong',
        positive: 'border-positive/35 bg-positive-soft text-positive',
        warning: 'border-warning/35 bg-warning-soft text-warning',
        negative: 'border-negative/35 bg-negative-soft text-negative',
      },
    },
    defaultVariants: { tone: 'neutral' },
  },
);

export interface BadgeProps extends VariantProps<typeof badge> {
  children: ReactNode;
  className?: string;
}

export function Badge({ tone, children, className }: BadgeProps) {
  return <span className={cn(badge({ tone }), className)}>{children}</span>;
}
