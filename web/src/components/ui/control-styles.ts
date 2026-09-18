/** Shared input/control appearance, used by Field and any bespoke control. */
export const controlStyles = [
  'xp-sunken bg-sunken text-ink w-full rounded-xs px-2.5 py-2 text-[13px] outline-none',
  'transition-[border-color,box-shadow,background-color] duration-150',
  'hover:border-accent-line focus:border-accent focus:bg-panel',
  'focus:shadow-[inset_0_0_0_1px_var(--color-accent-line)]',
  'aria-invalid:border-negative aria-invalid:bg-negative-soft',
].join(' ');
