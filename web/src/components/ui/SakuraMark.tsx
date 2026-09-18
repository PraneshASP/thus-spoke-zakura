import { cn } from '@/lib/cn';

const PETAL =
  'M12 12 C 8.6 11.2, 6.6 8.2, 7.3 5.3 C 7.7 3.7, 9.1 2.8, 10.3 3.6 L 12 5.4 L 13.7 3.6 C 14.9 2.8, 16.3 3.7, 16.7 5.3 C 17.4 8.2, 15.4 11.2, 12 12 Z';

/**
 * The project's namesake. Five notched petals drawn once and rotated, so the
 * blossom stays symmetrical at any size — unlike the icon-set flower it
 * replaces, which had no relationship to the brand.
 */
export function SakuraMark({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={cn('size-6', className)} role="img" aria-label="Zakura">
      <g fill="currentColor">
        {[0, 72, 144, 216, 288].map((angle) => (
          <path key={angle} d={PETAL} transform={`rotate(${angle} 12 12)`} />
        ))}
      </g>
      <circle cx="12" cy="12" r="1.6" className="fill-accent-strong/45" />
    </svg>
  );
}
