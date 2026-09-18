import { describe, expect, it, vi, afterEach } from 'vitest';
import { screen } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { renderWithProviders } from '@/test/utils';
import { ExplorerPage } from './ExplorerPage';

afterEach(() => {
  vi.restoreAllMocks();
});

function renderAt(path: string) {
  vi.spyOn(globalThis, 'fetch').mockImplementation(() =>
    Promise.resolve(
      new Response(JSON.stringify({ blocks: [], next_before: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      }),
    ),
  );

  return renderWithProviders(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/explorer/*" element={<ExplorerPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('ExplorerPage', () => {
  it('explains an unknown explorer path instead of rendering nothing', async () => {
    // `/explorer/*` swallows the app-level catch-all, so this page needs its
    // own fallback or the route is a silent dead end.
    renderAt('/explorer/does-not-exist');

    expect(await screen.findByRole('alert')).toHaveTextContent(/not part of the explorer/i);
    expect(screen.getByRole('link', { name: /back to blocks/i })).toBeInTheDocument();
  });

  it('still shows the block list at the explorer index', async () => {
    renderAt('/explorer');
    expect(await screen.findByLabelText('Search the chain')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
