import { useState } from 'react';
import { analyzeAll, analyzeWatermark, analyzePolez, analyzeStatistical, analyzeMetadata } from '../api/client';
import { useColorblind } from '../hooks/useColorblind';

interface Props {
  fileLoaded: boolean;
}

function ConfidenceBar({ label, value, max = 1 }: { label: string; value: number; max?: number }) {
  const { confidenceColor } = useColorblind();
  const pct = (value / max) * 100;
  const { bg, label: indicator } = confidenceColor(pct);

  return (
    <div className="mb-2">
      <div className="flex justify-between text-sm mb-1">
        <span className="text-zinc-300">{indicator ? `${indicator} ${label}` : label}</span>
        <span className="text-zinc-400">{(pct).toFixed(1)}%</span>
      </div>
      <div className="h-2 bg-zinc-700 rounded-full overflow-hidden" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} aria-label={`${label}: ${pct.toFixed(1)}%`}>
        <div className={`h-full ${bg} rounded-full`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnalysisResults = Record<string, any>;

export function DetectionPanel({ fileLoaded }: Props) {
  const { palette } = useColorblind();
  const [results, setResults] = useState<AnalysisResults | null>(null);
  const [loading, setLoading] = useState<string | null>(null);

  const runAnalysis = async (type: string) => {
    setLoading(type);
    try {
      let result: AnalysisResults;
      switch (type) {
        case 'all': result = await analyzeAll(); break;
        case 'watermark': result = { watermark: await analyzeWatermark() }; break;
        case 'polez': result = { polez: await analyzePolez() }; break;
        case 'statistical': result = { statistical: await analyzeStatistical() }; break;
        case 'metadata': result = { metadata: await analyzeMetadata() }; break;
        default: return;
      }
      setResults((prev) => ({ ...prev, ...result }));
    } finally {
      setLoading(null);
    }
  };

  return (
    <section className="bg-zinc-900 border border-zinc-700 rounded p-4" aria-label="Detection analysis">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <span className="text-zinc-400 text-sm font-medium">DETECTION</span>
        <div className="flex gap-2">
          <button
            data-action="detect"
            onClick={() => runAnalysis('all')}
            disabled={!fileLoaded || !!loading}
            className="bg-zinc-700 hover:bg-zinc-600 disabled:opacity-50 text-zinc-200 px-3 py-2 sm:py-1 rounded text-xs font-medium min-h-[44px] sm:min-h-0"
            aria-label="Run all detection analyses"
          >
            {loading === 'all' ? 'Running...' : 'Run All'}
          </button>
          <select
            onChange={(e) => { if (e.target.value) runAnalysis(e.target.value); e.target.value = ''; }}
            disabled={!fileLoaded || !!loading}
            className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-2 py-2 sm:py-1 text-xs min-h-[44px] sm:min-h-0"
            aria-label="Select specific analysis type"
          >
            <option value="">Pick Analysis...</option>
            <option value="watermark">Watermark</option>
            <option value="polez">Polez</option>
            <option value="statistical">Statistical</option>
            <option value="metadata">Metadata</option>
          </select>
        </div>
      </div>

      <div aria-live="polite">
      {!results && (
        <p className="text-zinc-500 text-sm">Load a file and run analysis</p>
      )}

      {results?.watermark && (
        <ConfidenceBar label="Watermark" value={results.watermark.overall_confidence} />
      )}
      {results?.polez && (
        <>
          <ConfidenceBar label="AI Watermark" value={results.polez.detection_probability} />
          <div className="text-xs text-zinc-500 mb-2 ml-2">
            Ultrasonic: {(results.polez.signals.ultrasonic_score * 100).toFixed(0)}% |
            Bit Planes: {results.polez.signals.biased_planes}/8 |
            Autocorr: {(results.polez.signals.autocorr_score * 100).toFixed(0)}%
          </div>
        </>
      )}
      {results?.statistical && (
        <ConfidenceBar label="AI Probability" value={results.statistical.ai_probability} />
      )}
      {results?.metadata && (
        <div className="text-sm text-zinc-300 mb-2">
          Metadata: {results.metadata.tags.length} tags, {results.metadata.suspicious_chunks.length} suspicious chunks
        </div>
      )}

      {results?.watermark?.method_results && (
        <details className="mt-3">
          <summary className="text-zinc-500 text-xs cursor-pointer hover:text-zinc-300">
            Expand method details...
          </summary>
          <div className="mt-2 space-y-1">
            {Object.entries(results.watermark.method_results as Record<string, { detected: boolean; confidence: number }>).map(([name, mr]) => (
              <div key={name} className="flex justify-between text-xs">
                <span className={mr.detected ? palette.detected.text : palette.notDetected.text}>{mr.detected ? '\u2717 ' : ''}{name}</span>
                <span className="text-zinc-400">{(mr.confidence * 100).toFixed(1)}%</span>
              </div>
            ))}
          </div>
        </details>
      )}
      </div>
    </section>
  );
}
