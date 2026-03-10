import { useEffect, useRef, useState, useCallback } from 'react';
import { getSpectrogram, type SpectrogramData } from '../api/client';

interface Props {
  fileLoaded: boolean;
}

interface ViewRange {
  freqMin: number;
  freqMax: number;
  timeStart: number;
  duration: number;
}

const DEFAULT_VIEW: ViewRange = {
  freqMin: 0,
  freqMax: 24000,
  timeStart: 0,
  duration: 0, // 0 means full duration
};

export function Spectrogram({ fileLoaded }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [data, setData] = useState<SpectrogramData | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<ViewRange>(DEFAULT_VIEW);
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef<{ x: number; y: number; view: ViewRange } | null>(null);

  const fetchData = useCallback(async () => {
    if (!fileLoaded) return;
    setLoading(true);
    try {
      const opts: Parameters<typeof getSpectrogram>[0] = {
        freq_min: view.freqMin,
        freq_max: view.freqMax,
      };
      if (view.timeStart > 0) opts.start = view.timeStart;
      if (view.duration > 0) opts.duration = view.duration;
      const d = await getSpectrogram(opts);
      setData(d);
    } finally {
      setLoading(false);
    }
  }, [fileLoaded, view]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  useEffect(() => {
    if (!data || !canvasRef.current) return;
    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const { magnitudes, num_time_frames, num_freq_bins } = data;
    canvas.width = num_time_frames;
    canvas.height = num_freq_bins;

    let minDb = 0, maxDb = -120;
    for (const row of magnitudes) {
      for (const val of row) {
        if (val > maxDb) maxDb = val;
        if (val < minDb && val > -120) minDb = val;
      }
    }

    const imageData = ctx.createImageData(num_time_frames, num_freq_bins);

    for (let t = 0; t < num_time_frames; t++) {
      for (let f = 0; f < num_freq_bins; f++) {
        const val = magnitudes[t][f];
        const norm = Math.max(0, Math.min(1, (val - minDb) / (maxDb - minDb)));

        const r = Math.floor(norm * 255);
        const g = Math.floor(norm * 128 + (1 - norm) * 20);
        const b = Math.floor((1 - norm) * 200 + norm * 50);

        const y = num_freq_bins - 1 - f;
        const idx = (y * num_time_frames + t) * 4;
        imageData.data[idx] = r;
        imageData.data[idx + 1] = g;
        imageData.data[idx + 2] = b;
        imageData.data[idx + 3] = 255;
      }
    }

    ctx.putImageData(imageData, 0, 0);
  }, [data]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    if (!data) return;

    const canvas = canvasRef.current;
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const xFrac = (e.clientX - rect.left) / rect.width;
    const yFrac = 1 - (e.clientY - rect.top) / rect.height;

    const zoomFactor = e.deltaY > 0 ? 1.2 : 0.8;

    setView(prev => {
      const freqRange = prev.freqMax - prev.freqMin;
      const timeRange = prev.duration > 0 ? prev.duration : (data.time_end - data.time_start);

      const newFreqRange = Math.max(500, Math.min(24000, freqRange * zoomFactor));
      const newTimeRange = Math.max(0.5, timeRange * zoomFactor);

      const freqCenter = prev.freqMin + freqRange * yFrac;
      const timeCenter = prev.timeStart + timeRange * xFrac;

      const newFreqMin = Math.max(0, freqCenter - newFreqRange * yFrac);
      const newFreqMax = Math.min(24000, newFreqMin + newFreqRange);
      const newTimeStart = Math.max(0, timeCenter - newTimeRange * xFrac);

      return {
        freqMin: Math.round(newFreqMin),
        freqMax: Math.round(newFreqMax),
        timeStart: Math.round(newTimeStart * 100) / 100,
        duration: Math.round(newTimeRange * 100) / 100,
      };
    });
  }, [data]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    setDragging(true);
    dragStart.current = { x: e.clientX, y: e.clientY, view: { ...view } };
  }, [view]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!dragging || !dragStart.current || !canvasRef.current || !data) return;

    const rect = canvasRef.current.getBoundingClientRect();
    const dx = (e.clientX - dragStart.current.x) / rect.width;
    const dy = (e.clientY - dragStart.current.y) / rect.height;

    const sv = dragStart.current.view;
    const freqRange = sv.freqMax - sv.freqMin;
    const timeRange = sv.duration > 0 ? sv.duration : (data.time_end - data.time_start);

    const newTimeStart = Math.max(0, sv.timeStart - dx * timeRange);
    const newFreqMin = Math.max(0, sv.freqMin + dy * freqRange);
    const newFreqMax = Math.min(24000, newFreqMin + freqRange);

    setView({
      freqMin: Math.round(newFreqMin),
      freqMax: Math.round(newFreqMax),
      timeStart: Math.round(newTimeStart * 100) / 100,
      duration: sv.duration,
    });
  }, [dragging, data]);

  const handleMouseUp = useCallback(() => {
    setDragging(false);
    dragStart.current = null;
  }, []);

  const resetZoom = useCallback(() => {
    setView(DEFAULT_VIEW);
  }, []);

  const isZoomed = view.freqMin !== 0 || view.freqMax !== 24000 || view.timeStart !== 0 || view.duration !== 0;

  return (
    <section className="bg-zinc-900 border border-zinc-700 rounded p-4" aria-label="Spectrogram display">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <span className="text-zinc-400 text-sm font-medium">SPECTROGRAM</span>
        <div className="flex items-center gap-2">
          {isZoomed && (
            <button
              onClick={resetZoom}
              className="text-zinc-500 hover:text-zinc-300 text-xs min-h-[44px] sm:min-h-0"
              aria-label="Reset spectrogram zoom"
            >
              Reset zoom
            </button>
          )}
          {loading && <span className="text-purple-400 text-xs" aria-live="polite">Loading...</span>}
          <span className="text-zinc-600 text-xs hidden sm:inline">Scroll to zoom, drag to pan</span>
        </div>
      </div>
      <div
        ref={containerRef}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        className="relative"
        style={{ cursor: dragging ? 'grabbing' : 'grab' }}
      >
        <canvas
          ref={canvasRef}
          className="w-full h-48 rounded"
          style={{ imageRendering: 'pixelated' }}
          aria-label="Spectrogram frequency visualization"
          role="img"
        />
      </div>
      {data && (
        <div className="flex justify-between text-zinc-500 text-xs mt-1">
          <span>{data.time_start.toFixed(1)}s</span>
          <span>{(data.freq_min / 1000).toFixed(1)}kHz - {(data.freq_max / 1000).toFixed(1)}kHz</span>
          <span>{data.time_end.toFixed(1)}s</span>
        </div>
      )}
    </section>
  );
}
