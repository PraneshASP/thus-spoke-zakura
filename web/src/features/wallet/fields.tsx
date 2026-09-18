import { Controller, type Control, type FieldPath, type FieldValues } from 'react-hook-form';
import { Field } from '@/components/ui/Field';
import { Select, type SelectOption } from '@/components/ui/Select';

/** A labelled Radix Select bound to a react-hook-form field. */
export function SelectField<TValues extends FieldValues, TContext, TTransformed>({
  control,
  name,
  label,
  hint,
  error,
  options,
}: {
  control: Control<TValues, TContext, TTransformed>;
  name: FieldPath<TValues>;
  label: string;
  hint?: string | undefined;
  error?: string | undefined;
  options: ReadonlyArray<SelectOption<string>>;
}) {
  return (
    <Field label={label} hint={hint} error={error}>
      {(aria) => (
        <Controller
          control={control}
          name={name}
          render={({ field }) => (
            <Select
              {...aria}
              options={options}
              value={String(field.value ?? '')}
              onValueChange={field.onChange}
            />
          )}
        />
      )}
    </Field>
  );
}
