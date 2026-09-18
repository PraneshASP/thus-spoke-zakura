import { cva, type VariantProps } from 'class-variance-authority';
import { Loader2 } from 'lucide-react';
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { cn } from '@/lib/cn';

const button = cva(
  [
    'inline-flex cursor-pointer items-center justify-center gap-2 rounded-xs font-medium',
    'xp-raised xp-press disabled:cursor-not-allowed disabled:opacity-55',
    '[&_svg]:size-4 [&_svg]:shrink-0',
  ],
  {
    variants: {
      variant: {
        primary:
          'xp-face-accent text-accent-ink font-semibold hover:brightness-108 active:brightness-97',
        ghost: 'xp-face text-ink hover:bg-accent-soft hover:bg-none',
        subtle:
          'xp-face text-ink-muted hover:bg-accent-soft hover:bg-none hover:text-accent-strong',
        danger: 'bg-negative text-white font-semibold',
      },
      size: {
        sm: 'px-3 py-2 text-[12px]',
        md: 'px-4 py-2.5 text-[13px]',
        block: 'w-full px-4 py-3 text-[14px]',
      },
    },
    defaultVariants: { variant: 'ghost', size: 'md' },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof button> {
  loading?: boolean;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant, size, loading = false, disabled, children, ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={props.type ?? 'button'}
      disabled={disabled ?? loading}
      aria-busy={loading || undefined}
      className={cn(button({ variant, size }), className)}
      {...props}
    >
      {loading && <Loader2 className="animate-spin" aria-hidden />}
      {children}
    </button>
  );
});
