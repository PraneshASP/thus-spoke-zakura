import { Skeleton } from '@/components/ui/Skeleton';

/**
 * Mirrors AccountCard's geometry so the grid does not reflow when real data
 * lands. `/accounts` syncs the wallet server-side and can take seconds, which
 * is long enough that a bare spinner reads as a stall.
 */
export function AccountCardSkeleton({ index = 0 }: { index?: number }) {
  return (
    <article
      className="xp-window animate-rise min-w-0 rounded-xs p-3.5"
      style={{ animationDelay: `${index * 45}ms` }}
      aria-hidden
    >
      <div className="flex items-center gap-2.5">
        <Skeleton className="size-7" />
        <div className="flex-1">
          <Skeleton className="h-3 w-24" />
          <Skeleton className="mt-1.5 h-2 w-16" />
        </div>
      </div>
      <div className="my-3.5 grid grid-cols-2 gap-2.5">
        <div>
          <Skeleton className="h-2 w-12" />
          <Skeleton className="mt-1.5 h-3 w-16" />
        </div>
        <div className="border-line border-l pl-3">
          <Skeleton className="h-2 w-16" />
          <Skeleton className="mt-1.5 h-3 w-14" />
        </div>
      </div>
      <Skeleton className="h-5 w-full" />
      <div className="mt-3 flex gap-2">
        <Skeleton className="h-7 flex-1" />
        <Skeleton className="h-7 flex-1" />
      </div>
    </article>
  );
}
