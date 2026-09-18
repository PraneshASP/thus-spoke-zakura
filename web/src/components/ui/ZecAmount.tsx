import { AnimatedValue } from './AnimatedValue';
import { useCountUp } from '@/lib/useCountUp';
import { formatZecAmount } from '@/lib/money';
import { cn } from '@/lib/cn';

/**
 * A balance that counts to its new value and briefly highlights when it
 * changes, so money moving is something you see rather than something you
 * have to notice.
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
  const displayed = useCountUp(zatoshi);

  return (
    <AnimatedValue value={zatoshi.toString()}>
      <span
        className={cn('tabular-nums', className, muteZero && zatoshi === 0n && 'text-ink-muted')}
      >
        {formatZecAmount(displayed)}
      </span>
    </AnimatedValue>
  );
}
