import { createContext, useContext, useState, useEffect, type ReactNode } from 'react';

interface ColorblindPalette {
  high: { bg: string; text: string; label: string };
  medium: { bg: string; text: string; label: string };
  low: { bg: string; text: string; label: string };
  biased: { bg: string; text: string };
  normal: { bg: string; text: string };
  grades: Record<string, string>;
  verdictColor: (color: string) => string;
  detected: { text: string };
  notDetected: { text: string };
}

const STANDARD_PALETTE: ColorblindPalette = {
  high: { bg: 'bg-red-500', text: 'text-red-400', label: '' },
  medium: { bg: 'bg-yellow-500', text: 'text-yellow-400', label: '' },
  low: { bg: 'bg-green-500', text: 'text-green-400', label: '' },
  biased: { bg: 'bg-red-500', text: 'text-red-400' },
  normal: { bg: 'bg-green-500', text: 'text-zinc-500' },
  grades: {
    A: 'text-green-400 border-green-500',
    B: 'text-emerald-400 border-emerald-500',
    C: 'text-yellow-400 border-yellow-500',
    D: 'text-orange-400 border-orange-500',
    F: 'text-red-400 border-red-500',
  },
  verdictColor: (color: string) =>
    color === 'green' ? 'text-green-400' : color === 'yellow' ? 'text-yellow-400' : 'text-red-400',
  detected: { text: 'text-red-400' },
  notDetected: { text: 'text-zinc-500' },
};

const COLORBLIND_PALETTE: ColorblindPalette = {
  high: { bg: 'bg-orange-500', text: 'text-orange-400', label: '\u25B2' },
  medium: { bg: 'bg-sky-500', text: 'text-sky-400', label: '\u25C6' },
  low: { bg: 'bg-violet-500', text: 'text-violet-400', label: '\u25CF' },
  biased: { bg: 'bg-orange-500', text: 'text-orange-400' },
  normal: { bg: 'bg-violet-500', text: 'text-zinc-500' },
  grades: {
    A: 'text-violet-400 border-violet-500',
    B: 'text-sky-400 border-sky-500',
    C: 'text-amber-400 border-amber-500',
    D: 'text-orange-400 border-orange-500',
    F: 'text-rose-400 border-rose-500',
  },
  verdictColor: (color: string) =>
    color === 'green' ? 'text-violet-400' : color === 'yellow' ? 'text-sky-400' : 'text-orange-400',
  detected: { text: 'text-orange-400' },
  notDetected: { text: 'text-zinc-500' },
};

interface ColorblindContext {
  enabled: boolean;
  toggle: () => void;
  palette: ColorblindPalette;
  confidenceColor: (pct: number) => { bg: string; text: string; label: string };
}

const Ctx = createContext<ColorblindContext>({
  enabled: false,
  toggle: () => {},
  palette: STANDARD_PALETTE,
  confidenceColor: () => STANDARD_PALETTE.low,
});

export function ColorblindProvider({ children }: { children: ReactNode }) {
  const [enabled, setEnabled] = useState(() => {
    try {
      return localStorage.getItem('polez-colorblind') === 'true';
    } catch {
      return false;
    }
  });

  useEffect(() => {
    const mq = window.matchMedia('(prefers-contrast: more)');
    if (mq.matches && !localStorage.getItem('polez-colorblind')) {
      setEnabled(true);
    }
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem('polez-colorblind', String(enabled));
    } catch {}
  }, [enabled]);

  const palette = enabled ? COLORBLIND_PALETTE : STANDARD_PALETTE;

  const confidenceColor = (pct: number) =>
    pct > 70 ? palette.high : pct > 40 ? palette.medium : palette.low;

  return (
    <Ctx.Provider value={{ enabled, toggle: () => setEnabled(v => !v), palette, confidenceColor }}>
      {children}
    </Ctx.Provider>
  );
}

export function useColorblind() {
  return useContext(Ctx);
}
