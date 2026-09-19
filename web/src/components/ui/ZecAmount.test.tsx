import { describe, expect, it } from 'vitest';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '@/test/utils';
import { ZecAmount } from './ZecAmount';

/**
 * `cn` resolves conflicting Tailwind utilities last-wins, so a caller's colour
 * class silently defeated `muteZero` when it was merged first. These assert the
 * ordering, not the styling: a zero balance must not keep the accent colour.
 */
describe('ZecAmount', () => {
  it('mutes a zero even when the caller passes its own colour', () => {
    renderWithProviders(<ZecAmount zatoshi={0n} className="text-accent-strong font-bold" />);
    const amount = screen.getByText(/0/);
    expect(amount.className).toContain('text-ink-muted');
    expect(amount.className).not.toContain('text-accent-strong');
    expect(amount.className).toContain('font-bold');
  });

  it('keeps the caller colour for a non-zero balance', () => {
    renderWithProviders(<ZecAmount zatoshi={500_000_000n} className="text-accent-strong" />);
    const amount = screen.getByText(/ZEC/);
    expect(amount.className).toContain('text-accent-strong');
    expect(amount.className).not.toContain('text-ink-muted');
  });

  it('honours muteZero={false}', () => {
    renderWithProviders(<ZecAmount zatoshi={0n} className="text-accent-strong" muteZero={false} />);
    expect(screen.getByText(/0/).className).toContain('text-accent-strong');
  });

  it('switches directly to the new balance without displaying intermediate amounts', () => {
    const view = renderWithProviders(<ZecAmount zatoshi={500_000_000n} />);

    view.rerender(<ZecAmount zatoshi={700_000_000n} />);

    expect(screen.getByText('7 ZEC')).toBeInTheDocument();
    expect(screen.queryByText('6 ZEC')).not.toBeInTheDocument();
  });
});
