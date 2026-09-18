import { useIsFetching, useIsMutating } from '@tanstack/react-query';
import { cn } from '@/lib/cn';

/**
 * A thin indeterminate bar shown while any query or mutation is in flight.
 *
 * The server syncs the wallet on /accounts and proves a transaction on /send,
 * so requests routinely take seconds. Without this the app looks idle while it
 * is working. Visibility is derived straight from the query cache — no state,
 * no effect — and the hide is delayed in CSS so quick successive fetches do
 * not make the bar flicker.
 */
export function FetchProgress() {
  const busy = useIsFetching() + useIsMutating() > 0;

  return (
    <div
      aria-hidden
      className={cn(
        'pointer-events-none fixed inset-x-0 top-0 z-100 h-0.5 overflow-hidden',
        'transition-opacity duration-200',
        busy ? 'opacity-100' : 'opacity-0 delay-300',
      )}
    >
      <span className="via-accent animate-sweep block h-full w-2/5 bg-linear-to-r from-transparent to-transparent" />
    </div>
  );
}
