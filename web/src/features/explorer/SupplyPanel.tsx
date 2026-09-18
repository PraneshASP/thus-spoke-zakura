import { Panel } from '@/components/ui/Panel';
import { formatZecAmount } from '@/lib/money';
import { cn } from '@/lib/cn';
import type { Block } from '@/lib/api';

const POOL_LABEL: Record<string, string> = {
  transparent: 'Transparent',
  orchard: 'Orchard',
  sapling: 'Sapling',
  sprout: 'Sprout',
  lockbox: 'Lockbox',
  ironwood: 'Ironwood',
};

/** Shielded pools, in the order the protocol introduced them. */
const SHIELDED = new Set(['sprout', 'sapling', 'orchard', 'ironwood']);

function signedZec(zat: number): string {
  const sign = zat > 0 ? '+' : zat < 0 ? '−' : '';
  return `${sign}${formatZecAmount(BigInt(Math.abs(zat)))}`;
}

/**
 * The split of total supply between shielded and transparent pools, as of this
 * block. It is the most Zcash-specific number the node reports and it arrives
 * in every block response, so the explorer shows it rather than discarding it.
 */
export function SupplyPanel({ block }: { block: Block }) {
  const total = block.chainSupply?.chainValueZat ?? 0;
  const pools = block.valuePools.filter((pool) => pool.chainValueZat > 0 || pool.monitored);
  const shielded = pools
    .filter((pool) => SHIELDED.has(pool.id))
    .reduce((sum, pool) => sum + pool.chainValueZat, 0);
  const shieldedPct = total > 0 ? (shielded / total) * 100 : 0;

  return (
    <Panel eyebrow="SUPPLY" title="Chain value at this block">
      <div className="border-line border-b px-5 py-4">
        <div className="flex items-baseline justify-between">
          <span className="text-ink text-[11px] font-bold tracking-[0.12em] uppercase">Total</span>
          <strong className="text-[15px] font-bold tabular-nums">
            {formatZecAmount(BigInt(total))}
          </strong>
        </div>

        {/* Shielded share of all coins in existence on this chain. */}
        <div
          className="xp-sunken bg-sunken mt-3 flex h-4 overflow-hidden rounded-xs"
          role="img"
          aria-label={`${shieldedPct.toFixed(2)}% of supply is shielded`}
        >
          <span className="bg-accent h-full" style={{ width: `${shieldedPct}%` }} />
        </div>
        <p className="text-ink-muted mt-1.5 text-[12px]">
          <b className="text-accent-strong font-bold">{shieldedPct.toFixed(2)}% shielded</b> ·{' '}
          {(100 - shieldedPct).toFixed(2)}% transparent
        </p>
      </div>

      <table className="w-full text-[13px]">
        <tbody>
          {pools.map((pool) => (
            <tr key={pool.id} className="border-line border-b last:border-b-0">
              <td className="px-5 py-3">
                <span
                  className={cn(
                    'text-[11px] font-bold tracking-[0.08em] uppercase',
                    SHIELDED.has(pool.id) ? 'text-accent' : 'text-ink-muted',
                  )}
                >
                  {POOL_LABEL[pool.id] ?? pool.id}
                </span>
              </td>
              <td className="px-5 py-3 text-right font-semibold tabular-nums">
                {formatZecAmount(BigInt(pool.chainValueZat))}
              </td>
              <td
                className={cn(
                  'w-32 px-5 py-3 text-right tabular-nums',
                  pool.valueDeltaZat > 0
                    ? 'text-positive'
                    : pool.valueDeltaZat < 0
                      ? 'text-negative'
                      : 'text-ink-muted',
                )}
                title="Change in this block"
              >
                {signedZec(pool.valueDeltaZat)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  );
}
