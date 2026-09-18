import { AlertTriangle, Loader2 } from 'lucide-react';
import type { ReactNode } from 'react';

/**
 * The previous UI rendered an empty array identically whether data was
 * loading, genuinely empty, or had failed, so a fetch error looked like an
 * empty wallet. These three states are now distinct.
 */
export function LoadingState({ label }: { label: string }) {
  return (
    <div
      className="text-ink-muted flex items-center justify-center gap-2.5 p-12 text-xs"
      role="status"
    >
      <Loader2 className="size-4 animate-spin" aria-hidden />
      {label}
    </div>
  );
}

export function ErrorState({ message, action }: { message: string; action?: ReactNode }) {
  return (
    <div
      role="alert"
      className="border-negative/35 bg-negative-soft flex flex-col items-center gap-3 rounded-xs border p-8 text-center"
    >
      <AlertTriangle className="text-negative size-5" aria-hidden />
      <p className="text-negative max-w-md text-[12px] leading-relaxed">{message}</p>
      {action}
    </div>
  );
}

export function EmptyState({ message }: { message: string }) {
  return <div className="text-ink-muted p-12 text-center text-xs">{message}</div>;
}
