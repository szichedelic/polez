/**
 * Polez design tokens.
 *
 * All UI chrome is neutral (zinc palette). Color exists only on data
 * elements: confidence bars, threat badges, waveform visualizations.
 */

export const tokens = {
  // ── Zinc neutral palette (UI chrome) ──────────────────────────
  bgPage: '#09090b',       // zinc-950
  bgCard: '#18181b',       // zinc-900
  border: '#27272a',       // zinc-800
  borderActive: '#3f3f46', // zinc-700
  textMuted: '#52525b',    // zinc-600
  textSecondary: '#a1a1aa', // zinc-400
  textPrimary: '#e4e4e7',  // zinc-200
  textEmphasis: '#fafafa', // zinc-50

  // ── Data-only accent colors (the 10% in 60-30-10) ────────────
  dataThreat: '#f43f5e',   // rose-500
  dataWarning: '#f59e0b',  // amber-500
  dataClean: '#10b981',    // emerald-500
  dataWaveform: '#a1a1aa', // zinc-400

  // ── Surface tokens ────────────────────────────────────────────
  radius: '6px',
  borderWidth: '1px',
  activeAccent: '2px',     // left border width for active states
} as const;

/** Inject design tokens as CSS custom properties on :root */
export function injectTokens(): void {
  const root = document.documentElement;
  root.style.setProperty('--bg-page', tokens.bgPage);
  root.style.setProperty('--bg-card', tokens.bgCard);
  root.style.setProperty('--border', tokens.border);
  root.style.setProperty('--border-active', tokens.borderActive);
  root.style.setProperty('--text-muted', tokens.textMuted);
  root.style.setProperty('--text-secondary', tokens.textSecondary);
  root.style.setProperty('--text-primary', tokens.textPrimary);
  root.style.setProperty('--text-emphasis', tokens.textEmphasis);
  root.style.setProperty('--data-threat', tokens.dataThreat);
  root.style.setProperty('--data-warning', tokens.dataWarning);
  root.style.setProperty('--data-clean', tokens.dataClean);
  root.style.setProperty('--data-waveform', tokens.dataWaveform);
  root.style.setProperty('--radius', tokens.radius);
  root.style.setProperty('--border-width', tokens.borderWidth);
  root.style.setProperty('--active-accent', tokens.activeAccent);
}
