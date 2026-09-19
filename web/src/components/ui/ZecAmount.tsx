import { AnimatedValue } from './AnimatedValue';
import { formatZecAmount } from '@/lib/money';
import { cn } from '@/lib/cn';

/**
 * A balance that switches directly to its new value and briefly highlights
 * when it changes. Interpolating money displays misleading intermediate
 * balances, especially when several whole ZEC arrive at once.
 */
export function ZecAmount({
  zatoshi,
  className,
  muteZero = true,
}: {
  zatoshi: bigint;
  className?: string;
  muteZero?: boolean;
}) {
  return (
    <AnimatedValue value={zatoshi.toString()}>
      <span
        className={cn('tabular-nums', className, muteZero && zatoshi === 0n && 'text-ink-muted')}
      >
        {formatZecAmount(zatoshi)}
      </span>
    </AnimatedValue>
  );
}
