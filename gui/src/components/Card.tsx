import type { ReactNode } from 'react';

type CardVariant = 'default' | 'active' | 'recessed';

interface Props {
  variant?: CardVariant;
  label: string;
  padding?: 'sm' | 'md';
  children: ReactNode;
}

const variantStyles: Record<CardVariant, string> = {
  default: 'bg-zinc-900 border border-zinc-800 rounded-[6px]',
  active: 'bg-zinc-900 border border-zinc-800 rounded-r-[6px] rounded-l-none border-l-2 border-l-zinc-700',
  recessed: 'bg-zinc-950 border border-dashed border-zinc-800 rounded-[6px]',
};

const paddingStyles = {
  sm: 'p-3',
  md: 'p-4',
};

export function Card({ variant = 'default', label, padding = 'md', children }: Props) {
  return (
    <section className={`${variantStyles[variant]} ${paddingStyles[padding]}`} aria-label={label}>
      {children}
    </section>
  );
}
