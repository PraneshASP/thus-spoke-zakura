import * as RadixSelect from '@radix-ui/react-select';
import { Check, ChevronDown } from 'lucide-react';
import { cn } from '@/lib/cn';
import { controlStyles } from './control-styles';

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

/**
 * A native <select> cannot be styled consistently across platforms, and a
 * hand-rolled listbox has to reimplement typeahead, roving focus and
 * collision-aware positioning. Radix provides all three.
 */
export function Select<T extends string>({
  value,
  onValueChange,
  options,
  id,
  ...aria
}: {
  value: T;
  onValueChange: (value: T) => void;
  options: ReadonlyArray<SelectOption<T>>;
  id?: string;
  'aria-invalid'?: boolean;
  'aria-describedby'?: string | undefined;
}) {
  return (
    <RadixSelect.Root value={value} onValueChange={onValueChange}>
      <RadixSelect.Trigger
        id={id}
        aria-invalid={aria['aria-invalid']}
        aria-describedby={aria['aria-describedby']}
        className={cn(controlStyles, 'flex cursor-pointer items-center justify-between gap-2')}
      >
        <RadixSelect.Value />
        <RadixSelect.Icon>
          <ChevronDown className="text-ink-muted size-4" />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>

      <RadixSelect.Portal>
        <RadixSelect.Content
          position="popper"
          sideOffset={6}
          className="xp-raised bg-panel z-60 min-w-(--radix-select-trigger-width) overflow-hidden rounded-xs shadow-(--shadow-pop)"
        >
          <RadixSelect.Viewport className="p-1">
            {options.map((option) => (
              <RadixSelect.Item
                key={option.value}
                value={option.value}
                className="text-ink data-[highlighted]:bg-accent data-[highlighted]:text-accent-ink data-[state=checked]:text-accent data-[highlighted]:data-[state=checked]:text-accent-ink flex cursor-pointer items-center justify-between gap-3 rounded-xs px-2.5 py-1.5 text-[13px] outline-none"
              >
                <RadixSelect.ItemText>{option.label}</RadixSelect.ItemText>
                <RadixSelect.ItemIndicator>
                  <Check className="text-accent size-3.5" />
                </RadixSelect.ItemIndicator>
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  );
}
