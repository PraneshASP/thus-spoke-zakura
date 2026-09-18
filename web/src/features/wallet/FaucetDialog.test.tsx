import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders, testAccounts } from '@/test/utils';
import { FaucetDialog } from './FaucetDialog';

describe('FaucetDialog', () => {
  it('exposes modal semantics the previous hand-rolled dialog lacked', () => {
    renderWithProviders(<FaucetDialog open onOpenChange={vi.fn()} accounts={testAccounts} />);

    const dialog = screen.getByRole('dialog');
    // The dialog is named and described by real elements, so screen readers
    // announce it. The previous implementation was an unlabelled <div>.
    const labelId = dialog.getAttribute('aria-labelledby');
    const descriptionId = dialog.getAttribute('aria-describedby');
    expect(labelId).toBeTruthy();
    expect(document.getElementById(labelId!)).toHaveTextContent('Fund an account');
    expect(descriptionId).toBeTruthy();
    expect(document.getElementById(descriptionId!)).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Fund an account' })).toBeInTheDocument();
  });

  it('moves focus into the dialog on open', () => {
    renderWithProviders(<FaucetDialog open onOpenChange={vi.fn()} accounts={testAccounts} />);
    expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
  });

  it('closes on Escape', async () => {
    const onOpenChange = vi.fn();
    renderWithProviders(<FaucetDialog open onOpenChange={onOpenChange} accounts={testAccounts} />);

    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(onOpenChange).toHaveBeenCalledWith(false));
  });

  it('defaults to 1 ZEC rather than the failure-prone 5 ZEC ceiling', () => {
    renderWithProviders(<FaucetDialog open onOpenChange={vi.fn()} accounts={testAccounts} />);
    expect(screen.getByLabelText('Amount (ZEC)')).toHaveValue('1');
  });

  it('rejects amounts above the server-side faucet limit before submitting', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    renderWithProviders(<FaucetDialog open onOpenChange={vi.fn()} accounts={testAccounts} />);

    const amount = screen.getByLabelText('Amount (ZEC)');
    await userEvent.clear(amount);
    await userEvent.type(amount, '6');
    await userEvent.click(screen.getByRole('button', { name: 'Add funds' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'The faucet is limited to 5 ZEC per request.',
    );
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });

  it('rejects non-numeric input', async () => {
    renderWithProviders(<FaucetDialog open onOpenChange={vi.fn()} accounts={testAccounts} />);

    const amount = screen.getByLabelText('Amount (ZEC)');
    await userEvent.clear(amount);
    await userEvent.type(amount, 'abc');
    await userEvent.click(screen.getByRole('button', { name: 'Add funds' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('up to 8 decimal places');
  });
});
