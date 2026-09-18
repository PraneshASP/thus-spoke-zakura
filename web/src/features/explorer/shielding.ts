import type { Transaction } from '@/lib/api';

export interface ShieldingSummary {
  orchardActions: number;
  transparentInputs: number;
  transparentOutputs: number;
  saplingSpends: number;
  saplingOutputs: number;
  /**
   * Net value entering or leaving the shielded pools. Public, and non-zero on
   * a shielded transfer that pays a fee.
   */
  valueBalanceZat: number;
  /** True when the transfer touches no transparent input or output. */
  shieldedOnly: boolean;
  /** True when nothing about the transfer is visible on-chain. */
  fullyShielded: boolean;
  /** True when value crosses the shielded/transparent boundary. */
  mixed: boolean;
}

/**
 * Summarises what a transaction actually reveals publicly. An Orchard-only
 * transfer hides sender, recipient and amount inside its actions — but the
 * pool value balance stays public, so a transfer that pays a fee still
 * discloses that fee. Only a zero balance reveals nothing at all.
 */
export function summariseShielding(tx: Transaction): ShieldingSummary {
  const orchardActions = tx.orchard?.actions.length ?? 0;
  const transparentInputs = tx.vin.length;
  const transparentOutputs = tx.vout.length;
  const saplingSpends = tx.vShieldedSpend.length;
  const saplingOutputs = tx.vShieldedOutput.length;

  const shieldedPresent = orchardActions > 0 || saplingSpends > 0 || saplingOutputs > 0;
  const transparentPresent = transparentInputs > 0 || transparentOutputs > 0;
  const valueBalanceZat = (tx.orchard?.valueBalanceZat ?? 0) + (tx.valueBalanceZat ?? 0);

  return {
    orchardActions,
    transparentInputs,
    transparentOutputs,
    saplingSpends,
    saplingOutputs,
    valueBalanceZat,
    shieldedOnly: shieldedPresent && !transparentPresent,
    fullyShielded: shieldedPresent && !transparentPresent && valueBalanceZat === 0,
    mixed: shieldedPresent && transparentPresent,
  };
}
