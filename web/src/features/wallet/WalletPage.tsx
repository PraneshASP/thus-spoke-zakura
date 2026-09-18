import { SakuraMark } from '@/components/ui/SakuraMark';
import { Send } from 'lucide-react';
import { useAccounts, useActivity } from '@/hooks/queries';
import { errorMessage } from '@/lib/api';
import { AccountCard } from './AccountCard';
import { ZecAmount } from '@/components/ui/ZecAmount';
import { PetalFall } from '@/components/ui/PetalFall';
import { ActivityList } from './ActivityList';
import { AccountCardSkeleton } from './AccountCardSkeleton';
import { Skeleton } from '@/components/ui/Skeleton';
import { useWalletActions } from './wallet-actions-context';
import { Button } from '@/components/ui/Button';
import { EmptyState, ErrorState } from '@/components/ui/StateBlock';

export function WalletPage() {
  const accounts = useAccounts();
  const activity = useActivity();
  const { openSend } = useWalletActions();

  const total = (accounts.data ?? []).reduce(
    (sum, account) => sum + account.orchard_zatoshi + account.transparent_zatoshi,
    0n,
  );

  return (
    <div className="grid min-w-0 gap-4">
      <section className="xp-window border-accent-line from-accent-soft animate-rise to-panel relative flex min-h-28 items-center overflow-hidden rounded-xs bg-linear-115 px-4 py-5 sm:px-6">
        <PetalFall count={10} />
        <div className="relative z-10 min-w-0 pr-16">
          <span className="text-ink-muted block text-[11px] font-medium tracking-[0.08em] uppercase">
            Total balance
          </span>
          {accounts.isSuccess ? (
            <strong className="text-accent-strong my-1 block text-xl leading-none font-semibold tracking-[-0.03em] break-all sm:text-[32px]">
              <ZecAmount zatoshi={total} muteZero={false} />
            </strong>
          ) : (
            <Skeleton className="my-1.5 h-8 w-44 max-w-full" />
          )}
          <small className="text-ink-muted block text-[12px]">
            Across 5 deterministic development accounts
          </small>
        </div>
        <SakuraMark className="text-petal animate-bloom pointer-events-none absolute -right-4 -bottom-6 size-24 opacity-70 sm:size-32" />
      </section>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-[14px] font-semibold">Accounts</h2>
          <p className="text-ink-muted mt-0.5 font-mono text-[11px]">
            ZIP-32 derived · Regtest only
          </p>
        </div>
        <Button variant="ghost" size="sm" onClick={() => openSend()}>
          <Send />
          Send ZEC
        </Button>
      </div>

      {accounts.isPending && (
        <section className="grid min-w-0 gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 5 }, (_, index) => (
            <AccountCardSkeleton key={index} index={index} />
          ))}
        </section>
      )}
      {accounts.isError && <ErrorState message={errorMessage(accounts.error)} />}
      {accounts.isSuccess &&
        (accounts.data.length === 0 ? (
          <EmptyState message="No development accounts were derived for this environment." />
        ) : (
          <section className="grid min-w-0 gap-2.5 sm:grid-cols-2 xl:grid-cols-3">
            {accounts.data.map((account, index) => (
              <AccountCard key={account.id} account={account} index={index} />
            ))}
          </section>
        ))}

      <ActivityList activity={activity} />
    </div>
  );
}
