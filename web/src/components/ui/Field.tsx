import { useId, type ReactNode } from 'react';
import { cn } from '@/lib/cn';

/**
 * Associates a label, control, hint and error message so the relationship is
 * exposed to assistive technology instead of being purely visual.
 */
export function Field({
  label,
  hint,
  error,
  children,
  className,
}: {
  label: string;
  hint?: string | undefined;
  error?: string | undefined;
  children: (props: {
    id: string;
    'aria-invalid': boolean;
    'aria-describedby': string | undefined;
  }) => ReactNode;
  className?: string;
}) {
  const id = useId();
  const messageId = `${id}-message`;
  const message = error ?? hint;

  return (
    <div className={cn('mb-4 grid gap-2', className)}>
      <label htmlFor={id} className="text-ink-muted text-[12px] font-medium">
        {label}
      </label>
      {children({
        id,
        'aria-invalid': Boolean(error),
        'aria-describedby': message ? messageId : undefined,
      })}
      {message && (
        <span
          id={messageId}
          role={error ? 'alert' : undefined}
          className={cn('text-[11px]', error ? 'text-negative' : 'text-ink-muted')}
        >
          {message}
        </span>
      )}
    </div>
  );
}
