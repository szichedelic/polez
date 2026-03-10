import { useState, useEffect } from 'react';
import { cleanFile, saveCleanedFile, getPresets, DEFAULT_ADVANCED_FLAGS, DEFAULT_FINGERPRINT_FLAGS } from '../api/client';
import type { CleanResponse, VerificationResult as VerResult, PresetInfo, AdvancedFlags, FingerprintFlags } from '../api/client';

interface Props {
  fileLoaded: boolean;
  onCleaned?: (result: CleanResponse) => void;
}

function ConfidenceBar({ label, value, max = 1 }: { label: string; value: number; max?: number }) {
  const pct = (value / max) * 100;
  const color = pct > 70 ? 'bg-red-500' : pct > 40 ? 'bg-yellow-500' : 'bg-green-500';

  return (
    <div className="mb-2">
      <div className="flex justify-between text-sm mb-1">
        <span className="text-zinc-300">{label}</span>
        <span className="text-zinc-400">{pct.toFixed(1)}%</span>
      </div>
      <div className="h-2 bg-zinc-700 rounded-full overflow-hidden">
        <div className={`h-full ${color} rounded-full`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

function DetectionColumn({ title, data }: { title: string; data: any }) {
  if (!data) return null;
  return (
    <div className="flex-1 min-w-0">
      <h4 className="text-xs font-medium text-zinc-400 mb-2 uppercase">{title}</h4>
      {data.watermark && (
        <ConfidenceBar label="Watermark" value={data.watermark.overall_confidence} />
      )}
      {data.polez && (
        <ConfidenceBar label="AI Watermark" value={data.polez.detection_probability} />
      )}
      {data.statistical && (
        <ConfidenceBar label="AI Probability" value={data.statistical.ai_probability} />
      )}
      {data.metadata && (
        <div className="text-sm text-zinc-300 mb-2">
          Metadata: {data.metadata.tags.length} tags, {data.metadata.suspicious_chunks.length} suspicious
        </div>
      )}
    </div>
  );
}

const GRADE_COLORS: Record<string, string> = {
  A: 'text-green-400 border-green-500',
  B: 'text-emerald-400 border-emerald-500',
  C: 'text-yellow-400 border-yellow-500',
  D: 'text-orange-400 border-orange-500',
  F: 'text-red-400 border-red-500',
};

function VerificationPanel({ verification: v }: { verification: VerResult }) {
  const gradeStyle = GRADE_COLORS[v.grade] || 'text-zinc-400 border-zinc-500';
  const snrDisplay = isFinite(v.snr_db) ? `${v.snr_db.toFixed(1)} dB` : 'Infinite';

  return (
    <div className="bg-zinc-800 border border-zinc-700 rounded p-3 mb-3">
      <div className="flex items-center gap-4 mb-3">
        <div className={`text-3xl font-bold border-2 rounded-lg w-12 h-12 flex items-center justify-center ${gradeStyle}`}>
          {v.grade}
        </div>
        <div>
          <div className="text-sm font-medium text-zinc-200">Quality Grade</div>
          <div className={`text-xs ${v.verdict_color === 'green' ? 'text-green-400' : v.verdict_color === 'yellow' ? 'text-yellow-400' : 'text-red-400'}`}>
            {v.verdict}
          </div>
        </div>
        <div className="ml-auto text-right">
          <div className="text-sm text-zinc-400">Threats</div>
          <div className="text-sm text-zinc-200">{v.original_threats} &rarr; {v.remaining_threats}</div>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-3">
        <div>
          <div className="text-xs text-zinc-500 mb-1">SNR</div>
          <div className="text-sm text-zinc-200">{snrDisplay}</div>
        </div>
        <div>
          <div className="text-xs text-zinc-500 mb-1">Spectral Similarity</div>
          <div className="text-sm text-zinc-200">{(v.spectral_similarity * 100).toFixed(1)}%</div>
        </div>
        <div>
          <div className="text-xs text-zinc-500 mb-1">Effectiveness</div>
          <div className="text-sm text-zinc-200">{v.removal_effectiveness.toFixed(1)}%</div>
        </div>
      </div>
    </div>
  );
}

const STEALTH_FLAG_LABELS: Record<keyof AdvancedFlags, string> = {
  phase_dither: 'Phase Dither',
  comb_mask: 'Comb Mask',
  transient_shift: 'Transient Shift',
  resample_nudge: 'Resample Nudge',
  phase_noise: 'Phase Noise',
  phase_swirl: 'Phase Swirl',
  masked_hf_phase: 'Masked HF Phase',
  gated_resample_nudge: 'Gated Resample Nudge',
  micro_eq_flutter: 'Micro EQ Flutter',
  hf_decorrelate: 'HF Decorrelate',
  refined_transient: 'Refined Transient',
  adaptive_transient: 'Adaptive Transient',
  adaptive_notch: 'Adaptive Notch',
};

const FP_FLAG_LABELS: Record<keyof FingerprintFlags, string> = {
  statistical_normalization: 'Statistical Normalization',
  temporal_randomization: 'Temporal Randomization',
  phase_randomization: 'Phase Randomization',
  micro_timing_perturbation: 'Micro Timing Perturbation',
  human_imperfections: 'Human Imperfections',
};

function FlagToggle({ label, checked, onChange, disabled }: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled: boolean;
}) {
  return (
    <label className="flex items-center gap-2 cursor-pointer">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
        className="accent-emerald-500"
      />
      <span className="text-xs text-zinc-300">{label}</span>
    </label>
  );
}

export function CleanPanel({ fileLoaded, onCleaned }: Props) {
  const [mode, setMode] = useState('standard');
  const [preset, setPreset] = useState('');
  const [presets, setPresets] = useState<PresetInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<CleanResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [advancedFlags, setAdvancedFlags] = useState<AdvancedFlags>({ ...DEFAULT_ADVANCED_FLAGS });
  const [fpFlags, setFpFlags] = useState<FingerprintFlags>({ ...DEFAULT_FINGERPRINT_FLAGS });

  useEffect(() => {
    getPresets().then(setPresets).catch(() => {});
  }, []);

  const handleClean = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await cleanFile(mode, preset || undefined, advancedFlags, fpFlags);
      setResult(res);
      onCleaned?.(res);
    } catch (e: any) {
      setError(e.message || 'Cleaning failed');
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveCleanedFile();
    } catch (e: any) {
      setError(e.message || 'Save failed');
    } finally {
      setSaving(false);
    }
  };

  const updateAdvFlag = (key: keyof AdvancedFlags, value: boolean) => {
    setAdvancedFlags(prev => ({ ...prev, [key]: value }));
  };

  const updateFpFlag = (key: keyof FingerprintFlags, value: boolean) => {
    setFpFlags(prev => ({ ...prev, [key]: value }));
  };

  return (
    <div className="bg-zinc-900 border border-zinc-700 rounded p-4">
      <div className="flex items-center justify-between mb-3">
        <span className="text-zinc-400 text-sm font-medium">CLEAN</span>
        <div className="flex gap-2 items-center">
          {presets.length > 0 && (
            <select
              value={preset}
              onChange={(e) => setPreset(e.target.value)}
              disabled={loading}
              className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-2 py-1 text-xs"
              title={preset ? presets.find(p => p.name === preset)?.description : 'No preset (use defaults)'}
            >
              <option value="">No Preset</option>
              {presets.map(p => (
                <option key={p.name} value={p.name}>{p.name}</option>
              ))}
            </select>
          )}
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value)}
            disabled={loading}
            className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-2 py-1 text-xs"
          >
            <option value="fast">Fast</option>
            <option value="standard">Standard</option>
            <option value="preserving">Preserving</option>
            <option value="aggressive">Aggressive</option>
          </select>
          <button
            data-action="clean"
            onClick={handleClean}
            disabled={!fileLoaded || loading}
            className="bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white px-3 py-1 rounded text-xs font-medium"
          >
            {loading ? 'Cleaning...' : 'Clean'}
          </button>
          {result && (
            <button
              data-action="save"
              onClick={handleSave}
              disabled={saving}
              className="bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white px-3 py-1 rounded text-xs font-medium"
            >
              {saving ? 'Saving...' : 'Save File'}
            </button>
          )}
        </div>
      </div>

      {error && (
        <p className="text-red-400 text-sm mb-2">{error}</p>
      )}

      {preset && presets.length > 0 && (
        <div className="text-xs text-zinc-500 mb-2">
          {(() => {
            const p = presets.find(x => x.name === preset);
            return p ? `${p.description} — paranoia: ${p.paranoia_level}, quality: ${p.preserve_quality}` : '';
          })()}
        </div>
      )}

      <button
        onClick={() => setShowAdvanced(!showAdvanced)}
        className="text-xs text-zinc-500 hover:text-zinc-300 mb-2 flex items-center gap-1"
      >
        <span>{showAdvanced ? '\u25BC' : '\u25B6'}</span>
        Advanced Options ({Object.values(advancedFlags).filter(Boolean).length + Object.values(fpFlags).filter(Boolean).length} active)
      </button>

      {showAdvanced && (
        <div className="bg-zinc-800 border border-zinc-700 rounded p-3 mb-3">
          <div className="mb-3">
            <h4 className="text-xs font-medium text-zinc-400 mb-2 uppercase">Stealth DSP Operations</h4>
            <div className="grid grid-cols-2 gap-1">
              {(Object.keys(STEALTH_FLAG_LABELS) as (keyof AdvancedFlags)[]).map(key => (
                <FlagToggle
                  key={key}
                  label={STEALTH_FLAG_LABELS[key]}
                  checked={advancedFlags[key]}
                  onChange={(v) => updateAdvFlag(key, v)}
                  disabled={loading}
                />
              ))}
            </div>
          </div>
          <div>
            <h4 className="text-xs font-medium text-zinc-400 mb-2 uppercase">Fingerprint Removal</h4>
            <div className="grid grid-cols-2 gap-1">
              {(Object.keys(FP_FLAG_LABELS) as (keyof FingerprintFlags)[]).map(key => (
                <FlagToggle
                  key={key}
                  label={FP_FLAG_LABELS[key]}
                  checked={fpFlags[key]}
                  onChange={(v) => updateFpFlag(key, v)}
                  disabled={loading}
                />
              ))}
            </div>
          </div>
        </div>
      )}

      {!result && !loading && !showAdvanced && (
        <p className="text-zinc-500 text-sm">Select a mode and click Clean to sanitize the loaded file</p>
      )}

      {loading && (
        <p className="text-zinc-400 text-sm">Running sanitization pipeline...</p>
      )}

      {result && (
        <>
          <div className="flex gap-4 text-xs text-zinc-400 mb-3">
            <span>Quality loss: {result.quality_loss.toFixed(2)}%</span>
            <span>Time: {result.processing_time.toFixed(1)}s</span>
            <span>Metadata removed: {result.metadata_removed}</span>
            <span>Patterns: {result.patterns_found} found, {result.patterns_suppressed} suppressed</span>
          </div>

          <VerificationPanel verification={result.verification} />

          <div className="flex gap-4">
            <DetectionColumn title="Before" data={result.before} />
            <div className="w-px bg-zinc-700" />
            <DetectionColumn title="After" data={result.after} />
          </div>
        </>
      )}
    </div>
  );
}
