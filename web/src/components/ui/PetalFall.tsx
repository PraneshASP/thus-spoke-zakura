import { useMemo, type CSSProperties } from 'react';
import { cn } from '@/lib/cn';

/**
 * Blossom petals drifting across the hero.
 *
 * Every value is derived from the petal index rather than Math.random, so the
 * field is stable across re-renders instead of reshuffling whenever the hero
 * re-renders. The layer is aria-hidden and disappears entirely under
 * `prefers-reduced-motion`, since it carries no information.
 */
function petalStyle(index: number): CSSProperties {
  // Golden-ratio spacing spreads petals horizontally without clustering.
  const left = (index * 61.803) % 100;
  const size = 9 + ((index * 5) % 8);
  const duration = 6 + ((index * 1.9) % 6);
  const delay = -((index * 2.3) % duration);
  const sway = (index % 2 === 0 ? 1 : -1) * (18 + ((index * 7) % 34));

  return {
    left: `${left.toFixed(2)}%`,
    width: `${size}px`,
    height: `${size}px`,
    animationName: 'drift',
    animationTimingFunction: 'linear',
    animationIterationCount: 'infinite',
    animationDuration: `${duration.toFixed(2)}s`,
    animationDelay: `${delay.toFixed(2)}s`,
    '--petal-drift': `${sway}px`,
    '--petal-fall': '175px',
    '--petal-spin': `${200 + ((index * 53) % 340)}deg`,
    '--petal-opacity': `${(0.45 + (index % 4) * 0.08).toFixed(2)}`,
  } as CSSProperties;
}

export function PetalFall({ count = 18, className }: { count?: number; className?: string }) {
  const petals = useMemo(
    () => Array.from({ length: count }, (_, index) => petalStyle(index)),
    [count],
  );

  return (
    <div
      aria-hidden
      className={cn(
        'pointer-events-none absolute inset-0 overflow-hidden motion-reduce:hidden',
        className,
      )}
    >
      {petals.map((style, index) => (
        <span
          key={index}
          style={style}
          className="bg-petal absolute top-0 block [border-radius:100%_0_100%_0]"
        />
      ))}
    </div>
  );
}
