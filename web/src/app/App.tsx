import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { Fuel, Pickaxe } from 'lucide-react';
import { Shell } from '@/components/layout/Shell';
import { WalletPage } from '@/features/wallet/WalletPage';
import { ExplorerPage } from '@/features/explorer/ExplorerPage';
import { NetworkPage } from '@/features/network/NetworkPage';
import { useServerEvents } from '@/hooks/useServerEvents';
import { useStatus } from '@/hooks/queries';
import { Button } from '@/components/ui/Button';
import { useWalletActions } from '@/features/wallet/wallet-actions-context';
import { ThemeToggle } from '@/components/ui/ThemeToggle';

const TITLES: Record<string, string> = {
  '/wallet': 'Wallet',
  '/explorer': 'Explorer',
  '/network': 'Network',
};

export function App() {
  useServerEvents();
  const { pathname } = useLocation();
  const { data: status } = useStatus();
  const instance = status?.instance;
  const { openFaucet, openMine } = useWalletActions();
  const section = `/${pathname.split('/')[1] ?? ''}`;
  const title = TITLES[section] ?? 'Wallet';
  const isWallet = section === '/wallet' || section === '/';
  // Detail views render their own <h1> (the block, transaction, address or
  // node). Rendering the section title as a second <h1> above it duplicated
  // the page heading and left two h1 elements in the document.
  const ownsHeading = /^\/explorer\/(block|tx|address)\//.test(pathname) || section === '/network';

  return (
    <Shell>
      <header className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          {/* Only worth showing when it disambiguates: the CLI defaults the
              instance to "default", and "INSTANCE / DEFAULT" is noise above
              every page for anyone running a single environment. */}
          {instance && instance !== 'default' && (
            <p className="text-accent mb-1.5 font-mono text-[11px] font-medium tracking-[0.13em]">
              INSTANCE / {instance.toUpperCase()}
            </p>
          )}
          {!ownsHeading && <h1 className="text-2xl font-semibold tracking-[-0.02em]">{title}</h1>}
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2.5">
          {/* Mining is chain-wide, so it is available everywhere. The faucet
              moves money into a wallet account, so it belongs to the wallet. */}
          <div className="w-28 md:hidden">
            <ThemeToggle />
          </div>
          <Button variant="ghost" onClick={openMine} className="max-sm:hidden">
            <Pickaxe />
            Mine
          </Button>
          {isWallet && (
            <Button variant="primary" onClick={() => openFaucet()}>
              <Fuel />
              Faucet
            </Button>
          )}
        </div>
      </header>

      <div key={pathname} className="animate-rise">
        <Routes>
          <Route path="/" element={<Navigate to="/wallet" replace />} />
          <Route path="/wallet" element={<WalletPage />} />
          <Route path="/explorer/*" element={<ExplorerPage />} />
          <Route path="/network" element={<NetworkPage />} />
          <Route path="*" element={<Navigate to="/wallet" replace />} />
        </Routes>
      </div>
    </Shell>
  );
}
