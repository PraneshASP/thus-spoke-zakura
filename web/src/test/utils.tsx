import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, type RenderResult } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';
import { ToastProvider } from '@/components/ui/Toast';
import type { Account } from '@/lib/api';

export function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === 'string') return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

export function renderWithProviders(ui: ReactElement): RenderResult {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>{children}</ToastProvider>
      </QueryClientProvider>
    );
  }

  return render(ui, { wrapper: Wrapper });
}

export const testAccounts: Account[] = [1, 2, 3, 4, 5].map((id) => ({
  id,
  name: `Account ${id}`,
  unified_address: `uregtest1account${id}`,
  transparent_address: `tmAccount${id}`,
  transparent_zatoshi: 0n,
  orchard_zatoshi: id === 1 ? 500_000_000n : 0n,
}));
