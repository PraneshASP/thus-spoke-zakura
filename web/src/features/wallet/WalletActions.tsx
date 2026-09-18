import { useMemo, useState, type ReactNode } from 'react';
import { WalletActionsContext, type WalletActionsApi } from './wallet-actions-context';
import { useAccounts } from '@/hooks/queries';
import { SendDialog } from './SendDialog';
import { FaucetDialog } from './FaucetDialog';
import { MineDialog } from './MineDialog';

type OpenDialog = 'send' | 'faucet' | 'mine' | null;

/**
 * Owns the transaction dialogs so the header and the account cards can trigger
 * them without threading state through the tree.
 */
export function WalletActionsProvider({ children }: { children: ReactNode }) {
  const [dialog, setDialog] = useState<OpenDialog>(null);
  const [faucetAccount, setFaucetAccount] = useState<number | undefined>(undefined);
  const [sendAccount, setSendAccount] = useState<number | undefined>(undefined);
  const { data: accounts = [] } = useAccounts();

  const api = useMemo<WalletActionsApi>(
    () => ({
      openSend: (accountId) => {
        setSendAccount(accountId);
        setDialog('send');
      },
      openFaucet: (accountId) => {
        setFaucetAccount(accountId);
        setDialog('faucet');
      },
      openMine: () => setDialog('mine'),
    }),
    [],
  );

  const close = (open: boolean) => {
    if (!open) setDialog(null);
  };

  return (
    <WalletActionsContext value={api}>
      {children}
      {/* Keyed so each dialog remounts with fresh defaults per open. */}
      {dialog === 'send' && (
        <SendDialog
          key={`send-${sendAccount ?? 'any'}`}
          open
          onOpenChange={close}
          accounts={accounts}
          {...(sendAccount === undefined ? {} : { defaultAccountId: sendAccount })}
        />
      )}
      {dialog === 'faucet' && (
        <FaucetDialog
          key={`faucet-${faucetAccount ?? 'any'}`}
          open
          onOpenChange={close}
          accounts={accounts}
          {...(faucetAccount === undefined ? {} : { defaultAccountId: faucetAccount })}
        />
      )}
      {dialog === 'mine' && <MineDialog key="mine" open onOpenChange={close} />}
    </WalletActionsContext>
  );
}
