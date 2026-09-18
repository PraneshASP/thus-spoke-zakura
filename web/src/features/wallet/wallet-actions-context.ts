import { createContext, use } from 'react';

export interface WalletActionsApi {
  openSend: (accountId?: number) => void;
  openFaucet: (accountId?: number) => void;
  openMine: () => void;
}

export const WalletActionsContext = createContext<WalletActionsApi | null>(null);

export function useWalletActions(): WalletActionsApi {
  const api = use(WalletActionsContext);
  if (!api) throw new Error('useWalletActions must be used inside <WalletActionsProvider>');
  return api;
}
