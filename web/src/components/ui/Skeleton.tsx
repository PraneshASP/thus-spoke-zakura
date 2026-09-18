import { cn } from '@/lib/cn';

/** Placeholder matching the shape of the content it stands in for. */
export function Skeleton({ className }: { className?: string }) {
  return <span className={cn('skeleton block rounded-xs', className)} aria-hidden />;
}

export function SkeletonRows({ rows = 4, className }: { rows?: number; className?: string }) {
  return (
    <div className={cn('grid gap-2 p-4', className)} role="status" aria-label="Loading">
      {Array.from({ length: rows }, (_, index) => (
        <Skeleton key={index} className="h-8" />
      ))}
    </div>
  );
}
