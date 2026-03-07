import { useState } from 'react';
import { cleanFile, saveCleanedFile } from '../api/client';
import type { CleanResponse } from '../api/client';

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

export function CleanPanel({ fileLoaded, onCleaned }: Props) {
  const [mode, setMode] = useState('standard');
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [result, setResult] = useState<CleanResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleClean = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await cleanFile(mode);
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

  return (
    <div className="bg-zinc-900 border border-zinc-700 rounded p-4">
      <div className="flex items-center justify-between mb-3">
        <span className="text-zinc-400 text-sm font-medium">CLEAN</span>
        <div className="flex gap-2 items-center">
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
            onClick={handleClean}
            disabled={!fileLoaded || loading}
            className="bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white px-3 py-1 rounded text-xs font-medium"
          >
            {loading ? 'Cleaning...' : 'Clean'}
          </button>
          {result && (
            <button
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

      {!result && !loading && (
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
