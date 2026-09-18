import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import { Pickaxe } from 'lucide-react';
import { Dialog } from '@/components/ui/Dialog';
import { Button } from '@/components/ui/Button';
import { Field } from '@/components/ui/Field';
import { useToast } from '@/components/ui/toast-context';
import { errorMessage } from '@/lib/api';
import { useMine } from '@/hooks/mutations';
import { mineSchema, type MineInput, type MineValues } from './schemas';
import { controlStyles } from '@/components/ui/control-styles';

export function MineDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const toast = useToast();
  const mine = useMine();

  const form = useForm<MineInput, unknown, MineValues>({
    resolver: zodResolver(mineSchema),
    defaultValues: { blocks: '1' },
  });

  const submit = form.handleSubmit((values) => {
    mine.mutate(values.blocks, {
      onSuccess: (result) => {
        toast.success(
          `Mined ${result.blocks.toLocaleString()} block${result.blocks === 1 ? '' : 's'}`,
        );
        onOpenChange(false);
        form.reset();
      },
      onError: (error) => toast.error('Mining failed', errorMessage(error)),
    });
  });

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      eyebrow="CHAIN CONTROL"
      title="Mine blocks"
      description="Regtest mines on demand, with no proof-of-work delay."
    >
      <form onSubmit={(event) => void submit(event)} noValidate>
        <Field
          label="Number of blocks"
          hint="Between 1 and 10,000."
          error={form.formState.errors.blocks?.message}
        >
          {(aria) => (
            <input
              {...aria}
              {...form.register('blocks')}
              inputMode="numeric"
              autoComplete="off"
              className={controlStyles}
            />
          )}
        </Field>

        <Button type="submit" variant="primary" size="block" loading={mine.isPending}>
          <Pickaxe />
          {mine.isPending ? 'Mining…' : 'Mine blocks'}
        </Button>
      </form>
    </Dialog>
  );
}
