import type { ButtonHTMLAttributes, ReactNode } from 'react';

type ButtonVariant = 'primary' | 'secondary' | 'ghost';

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
}

const base =
  'rounded-[6px] px-[0.9rem] py-[0.45rem] text-[0.7rem] font-medium min-h-[44px] sm:min-h-0 transition-colors';

const variants: Record<ButtonVariant, string> = {
  primary:
    'bg-zinc-50 text-zinc-950 hover:bg-zinc-200 disabled:bg-zinc-900 disabled:text-zinc-700 disabled:cursor-not-allowed',
  secondary:
    'bg-zinc-800 text-zinc-200 hover:bg-zinc-700 disabled:bg-zinc-900 disabled:text-zinc-700 disabled:cursor-not-allowed',
  ghost:
    'bg-transparent border border-zinc-800 text-zinc-500 hover:border-zinc-700 hover:text-zinc-400 disabled:bg-transparent disabled:border-zinc-900 disabled:text-zinc-700 disabled:cursor-not-allowed',
};

export function Button({ variant = 'secondary', className, children, ...props }: Props) {
  return (
    <button className={`${base} ${variants[variant]} ${className ?? ''}`} {...props}>
      {children}
    </button>
  );
}
