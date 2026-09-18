import { useEffect, useRef, useState, type ReactNode } from 'react';
import { cn } from '@/lib/cn';

/**
 * Briefly highlights its contents whenever the underlying value changes, so a
 * balance updating after a send is noticed rather than silently replaced.
 */
export function AnimatedValue({
  value,
  className,
  children,
}: {
  value: string;
  className?: string;
  children: ReactNode;
}) {
  const previous = useRef(value);
  const [changed, setChanged] = useState(false);

  useEffect(() => {
    if (previous.current === value) return;
    previous.current = value;
    setChanged(true);
    const timer = setTimeout(() => setChanged(false), 1100);
    return () => clearTimeout(timer);
  }, [value]);

  return (
    <span
      className={cn('rounded-xs px-0.5 transition-colors', changed && 'animate-flash', className)}
    >
      {children}
    </span>
  );
}
