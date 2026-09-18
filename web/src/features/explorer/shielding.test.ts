import { describe, expect, it } from 'vitest';
import { summariseShielding } from './shielding';
import type { Transaction, TxOutput } from '@/lib/api';

const output = (valueZat = 0): TxOutput => ({ n: 0, valueZat });

const base: Transaction = {
  txid: 'a'.repeat(64),
  vin: [],
  vout: [],
  vShieldedSpend: [],
  vShieldedOutput: [],
};

describe('summariseShielding', () => {
  it('recognises a fully shielded Orchard transfer', () => {
    // Shape taken from a real /transactions response on a live regtest.
    const summary = summariseShielding({ ...base, orchard: { actions: [{}, {}] } });
    expect(summary.orchardActions).toBe(2);
    expect(summary.transparentInputs).toBe(0);
    expect(summary.transparentOutputs).toBe(0);
    expect(summary.fullyShielded).toBe(true);
    expect(summary.mixed).toBe(false);
  });

  it('recognises a shielding/deshielding transaction as mixed', () => {
    const summary = summariseShielding({
      ...base,
      vout: [output(39_072_500)],
      orchard: { actions: [{}, {}] },
    });
    expect(summary.fullyShielded).toBe(false);
    expect(summary.mixed).toBe(true);
  });

  it('treats a transparent-only transaction as neither', () => {
    const summary = summariseShielding({ ...base, vin: [{}], vout: [output(), output()] });
    expect(summary.fullyShielded).toBe(false);
    expect(summary.mixed).toBe(false);
    expect(summary.transparentOutputs).toBe(2);
  });

  it('does not claim full shielding when the pool value balance is public', () => {
    // No transparent inputs or outputs, but a non-zero Orchard value balance:
    // the fee is visible on chain even though sender, recipient and amount are
    // not. ZIP 224 makes this field part of the transaction.
    const summary = summariseShielding({
      txid: 'ab'.repeat(32),
      vin: [],
      vout: [],
      vShieldedSpend: [],
      vShieldedOutput: [],
      orchard: { actions: [{}, {}], valueBalanceZat: 10_000 },
    });

    expect(summary.shieldedOnly).toBe(true);
    expect(summary.fullyShielded).toBe(false);
    expect(summary.valueBalanceZat).toBe(10_000);
  });

  it('claims full shielding only when the balance is zero too', () => {
    const summary = summariseShielding({
      txid: 'ab'.repeat(32),
      vin: [],
      vout: [],
      vShieldedSpend: [],
      vShieldedOutput: [],
      orchard: { actions: [{}, {}], valueBalanceZat: 0 },
    });

    expect(summary.fullyShielded).toBe(true);
  });
});
