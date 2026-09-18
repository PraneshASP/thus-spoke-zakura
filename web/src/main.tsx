import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { App } from '@/app/App';
import { AppProviders } from '@/app/providers';
import { ToastProvider } from '@/components/ui/Toast';
import { FetchProgress } from '@/components/ui/FetchProgress';
import { WalletActionsProvider } from '@/features/wallet/WalletActions';
import '@/styles/globals.css';

const container = document.getElementById('root');
if (!container) throw new Error('Missing #root element');

createRoot(container).render(
  <StrictMode>
    <AppProviders>
      <ToastProvider>
        <FetchProgress />
        <BrowserRouter>
          <WalletActionsProvider>
            <App />
          </WalletActionsProvider>
        </BrowserRouter>
      </ToastProvider>
    </AppProviders>
  </StrictMode>,
);
