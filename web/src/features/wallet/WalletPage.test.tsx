import { describe, expect, it, vi, afterEach } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { renderWithProviders, requestUrl } from '@/test/utils';
import { WalletActionsProvider } from './WalletActions';
import { WalletPage } from './WalletPage';

const TXID = 'ab'.repeat(32);

function json(body: unknown): Promise<Response> {
  return Promise.resolve(
    new Response(JSON.stringify(body), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    }),
  );
}

function mockWalletApis() {
  vi.spyOn(globalThis, 'fetch').mockImplementation((input) => {
    const url = requestUrl(input);
    if (url.includes('/accounts')) {
      return json([
        {
          id: 1,
          name: 'Account 1',
          unified_address: 'uregtest1account1verylongunifiedaddressfortruncation',
          transparent_address: 'tmAccount1',
          transparent_zatoshi: 0,
          orchard_zatoshi: 500_000_000,
        },
      ]);
    }
    if (url.includes('/activity')) {
      return json([
        {
          id: 'act-1',
          kind: 'faucet',
          from_account: null,
          to_account: 1,
          source_pool: 'orchard',
          destination_pool: 'orchard',
          amount_zatoshi: 500_000_000,
          txid: TXID,
          block_hash: 'cd'.repeat(32),
          status: 'confirmed',
          created_at: '2026-09-18 10:00:00',
        },
      ]);
    }
    return json({
      instance: 'default',
      node: null,
      account_count: 5,
      auto_mine: true,
      network: 'regtest',
    });
  });
}

function renderWallet() {
  mockWalletApis();
  return renderWithProviders(
    <MemoryRouter>
      <WalletActionsProvider>
        <WalletPage />
      </WalletActionsProvider>
    </MemoryRouter>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('WalletPage', () => {
  it('uses the wrapping page rhythm the explorer and network surfaces share', async () => {
    const { container } = renderWallet();
    await waitFor(() => expect(screen.getByText('Account 1')).toBeInTheDocument());

    expect(container.firstElementChild).toHaveClass('grid', 'gap-4');
    expect(screen.getByRole('button', { name: 'Send ZEC' }).parentElement).toHaveClass('flex-wrap');
  });

  it('does not dump a full transaction id into the activity table', async () => {
    renderWallet();
    const tx = await screen.findByTitle(TXID);
    expect(tx).not.toHaveTextContent(TXID);
    expect(tx.textContent?.includes('…')).toBe(true);
  });

  it('counts a single activity row in the singular', async () => {
    renderWallet();
    expect(await screen.findByText('1 event')).toBeInTheDocument();
  });
});
