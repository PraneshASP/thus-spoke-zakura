import { describe, expect, it } from 'vitest';
import type { TxInput } from '@/lib/api';
import { resolveTransparentInput } from './resolve-input';

describe('resolveTransparentInput', () => {
  it('reads the address and value the server copied onto the input', () => {
    const input: TxInput = {
      txid: 'ab'.repeat(32),
      vout: 0,
      valueZat: 100_000_000,
      scriptPubKey: { addresses: ['tmSpentOutput'] },
    };
    expect(resolveTransparentInput(input)).toEqual({
      coinbase: false,
      prevTxid: 'ab'.repeat(32),
      address: 'tmSpentOutput',
      valueZat: 100_000_000,
    });
  });

  it('still names the previous transaction when the server could not enrich it', () => {
    expect(resolveTransparentInput({ txid: 'cd'.repeat(32), vout: 1 })).toEqual({
      coinbase: false,
      prevTxid: 'cd'.repeat(32),
    });
  });

  it('does not invent an amount for coinbase', () => {
    expect(resolveTransparentInput({ coinbase: '03' })).toEqual({ coinbase: true });
  });
});
