import type { TxInput } from '@/lib/api';

export interface ResolvedTransparentInput {
  coinbase: boolean;
  prevTxid?: string;
  address?: string;
  valueZat?: number;
}

/**
 * Node vin is a bare outpoint. The server copies the spent output's address and
 * value onto it before serving the transaction, so this only has to read them.
 */
export function resolveTransparentInput(input: TxInput): ResolvedTransparentInput {
  if (input.coinbase) return { coinbase: true };

  const address = input.scriptPubKey?.addresses[0];
  const valueZat = input.valueZat;
  return {
    coinbase: false,
    ...(input.txid ? { prevTxid: input.txid } : {}),
    ...(address ? { address } : {}),
    ...(valueZat !== undefined ? { valueZat } : {}),
  };
}
