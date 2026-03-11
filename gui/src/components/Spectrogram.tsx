import { useEffect, useRef, useState, useCallback } from 'react';
import { getSpectrogram, type SpectrogramData } from '../api/client';
import { Card } from './Card';

// Viridis colormap — 16 control points, linearly interpolated to 256
const VIRIDIS_CTRL: [number, number, number][] = [
  [68,1,84],[72,26,108],[71,47,126],[65,68,135],[57,86,140],
  [49,104,142],[42,120,142],[35,137,142],[30,152,138],[34,168,132],
  [53,183,121],[80,196,106],[115,208,86],[158,217,59],[204,225,30],[253,231,37],
];

function buildViridisLUT(): Uint8Array {
  const lut = new Uint8Array(256 * 3);
  for (let i = 0; i < 256; i++) {
    const t = (i / 255) * (VIRIDIS_CTRL.length - 1);
    const lo = Math.floor(t);
    const hi = Math.min(lo + 1, VIRIDIS_CTRL.length - 1);
    const f = t - lo;
    lut[i * 3]     = Math.round(VIRIDIS_CTRL[lo][0] + (VIRIDIS_CTRL[hi][0] - VIRIDIS_CTRL[lo][0]) * f);
    lut[i * 3 + 1] = Math.round(VIRIDIS_CTRL[lo][1] + (VIRIDIS_CTRL[hi][1] - VIRIDIS_CTRL[lo][1]) * f);
    lut[i * 3 + 2] = Math.round(VIRIDIS_CTRL[lo][2] + (VIRIDIS_CTRL[hi][2] - VIRIDIS_CTRL[lo][2]) * f);
  }
  return lut;
}

const VIRIDIS = buildViridisLUT();

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
  duration: 0,
};

export function Spectrogram({ fileLoaded }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [data, setData] = useState<SpectrogramData | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<ViewRange>(DEFAULT_VIEW);
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef<{ x: number; y: number; view: ViewRange } | null>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [debouncedView, setDebouncedView] = useState<ViewRange>(DEFAULT_VIEW);
  const [fullDuration, setFullDuration] = useState(0);
  const [dbRange, setDbRange] = useState<[number, number]>([0, 0]);
  const [cssTransform, setCssTransform] = useState('');
  const prevViewRef = useRef<ViewRange>(DEFAULT_VIEW);
  const pinchStartDist = useRef<number | null>(null);
  const pinchStartView = useRef<ViewRange | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      setDebouncedView(view);
    }, 200);
    return () => { if (debounceRef.current) clearTimeout(debounceRef.current); };
  }, [view]);

  const fetchData = useCallback(async () => {
    if (!fileLoaded) return;
    setLoading(true);
    try {
      const opts: Parameters<typeof getSpectrogram>[0] = {
        freq_min: debouncedView.freqMin,
        freq_max: debouncedView.freqMax,
      };
      if (debouncedView.timeStart > 0) opts.start = debouncedView.timeStart;
      if (debouncedView.duration > 0) opts.duration = debouncedView.duration;
      const d = await getSpectrogram(opts);
      setData(d);
      if (debouncedView.duration === 0 && debouncedView.timeStart === 0) {
        setFullDuration(d.time_end - d.time_start);
      }
      setCssTransform('');
      prevViewRef.current = debouncedView;
    } finally {
      setLoading(false);
    }
  }, [fileLoaded, debouncedView]);

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
    setDbRange([minDb, maxDb]);

    const imageData = ctx.createImageData(num_time_frames, num_freq_bins);

    for (let t = 0; t < num_time_frames; t++) {
      for (let f = 0; f < num_freq_bins; f++) {
        const val = magnitudes[t][f];
        const norm = Math.max(0, Math.min(1, (val - minDb) / (maxDb - minDb)));
        const ci = Math.round(norm * 255) * 3;

        const y = num_freq_bins - 1 - f;
        const idx = (y * num_time_frames + t) * 4;
        imageData.data[idx]     = VIRIDIS[ci];
        imageData.data[idx + 1] = VIRIDIS[ci + 1];
        imageData.data[idx + 2] = VIRIDIS[ci + 2];
        imageData.data[idx + 3] = 255;
      }
    }

    ctx.putImageData(imageData, 0, 0);
  }, [data]);

  const dataRef = useRef(data);
  dataRef.current = data;

  // Compute CSS transform for instant visual feedback during zoom/pan
  const applyCssTransform = useCallback((newView: ViewRange) => {
    const prev = prevViewRef.current;
    const d = dataRef.current;
    if (!d) return;

    const prevTimeRange = prev.duration > 0 ? prev.duration : (d.time_end - d.time_start);
    const newTimeRange = newView.duration > 0 ? newView.duration : prevTimeRange;
    const prevFreqRange = prev.freqMax - prev.freqMin;
    const newFreqRange = newView.freqMax - newView.freqMin;

    const scaleX = prevTimeRange / newTimeRange;
    const scaleY = prevFreqRange / newFreqRange;
    const translateX = -((newView.timeStart - prev.timeStart) / prevTimeRange) * 100;
    const translateY = ((newView.freqMin - prev.freqMin) / prevFreqRange) * 100;

    setCssTransform(`translate(${translateX}%, ${translateY}%) scale(${scaleX}, ${scaleY})`);
  }, []);

  // Wheel zoom
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const d = dataRef.current;
      if (!d) return;

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const xFrac = (e.clientX - rect.left) / rect.width;
      const yFrac = 1 - (e.clientY - rect.top) / rect.height;

      const zoomFactor = e.deltaY > 0 ? 1.2 : 0.8;

      setView(prev => {
        const freqRange = prev.freqMax - prev.freqMin;
        const timeRange = prev.duration > 0 ? prev.duration : (d.time_end - d.time_start);

        const newFreqRange = Math.max(500, Math.min(24000, freqRange * zoomFactor));
        const newTimeRange = Math.max(0.5, timeRange * zoomFactor);

        const freqCenter = prev.freqMin + freqRange * yFrac;
        const timeCenter = prev.timeStart + timeRange * xFrac;

        const newFreqMin = Math.max(0, freqCenter - newFreqRange * yFrac);
        const newFreqMax = Math.min(24000, newFreqMin + newFreqRange);
        const newTimeStart = Math.max(0, timeCenter - newTimeRange * xFrac);

        const newView = {
          freqMin: Math.round(newFreqMin),
          freqMax: Math.round(newFreqMax),
          timeStart: Math.round(newTimeStart * 100) / 100,
          duration: Math.round(newTimeRange * 100) / 100,
        };
        applyCssTransform(newView);
        return newView;
      });
    };

    container.addEventListener('wheel', onWheel, { passive: false });
    return () => container.removeEventListener('wheel', onWheel);
  }, [applyCssTransform]);

  // Touch pinch-to-zoom
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const getTouchDist = (e: TouchEvent) => {
      const [a, b] = [e.touches[0], e.touches[1]];
      return Math.hypot(b.clientX - a.clientX, b.clientY - a.clientY);
    };

    const onTouchStart = (e: TouchEvent) => {
      if (e.touches.length === 2) {
        e.preventDefault();
        pinchStartDist.current = getTouchDist(e);
        setView(prev => { pinchStartView.current = { ...prev }; return prev; });
      }
    };

    const onTouchMove = (e: TouchEvent) => {
      if (e.touches.length === 2 && pinchStartDist.current && pinchStartView.current) {
        e.preventDefault();
        const d = dataRef.current;
        if (!d) return;

        const currentDist = getTouchDist(e);
        const scale = pinchStartDist.current / currentDist;
        const sv = pinchStartView.current;
        const timeRange = sv.duration > 0 ? sv.duration : (d.time_end - d.time_start);
        const freqRange = sv.freqMax - sv.freqMin;

        const newTimeRange = Math.max(0.5, timeRange * scale);
        const newFreqRange = Math.max(500, Math.min(24000, freqRange * scale));

        const timeMid = sv.timeStart + timeRange / 2;
        const freqMid = sv.freqMin + freqRange / 2;

        const newTimeStart = Math.max(0, timeMid - newTimeRange / 2);
        const newFreqMin = Math.max(0, freqMid - newFreqRange / 2);
        const newFreqMax = Math.min(24000, newFreqMin + newFreqRange);

        const newView = {
          freqMin: Math.round(newFreqMin),
          freqMax: Math.round(newFreqMax),
          timeStart: Math.round(newTimeStart * 100) / 100,
          duration: Math.round(newTimeRange * 100) / 100,
        };
        applyCssTransform(newView);
        setView(newView);
      }
    };

    const onTouchEnd = () => {
      pinchStartDist.current = null;
      pinchStartView.current = null;
    };

    container.addEventListener('touchstart', onTouchStart, { passive: false });
    container.addEventListener('touchmove', onTouchMove, { passive: false });
    container.addEventListener('touchend', onTouchEnd);
    return () => {
      container.removeEventListener('touchstart', onTouchStart);
      container.removeEventListener('touchmove', onTouchMove);
      container.removeEventListener('touchend', onTouchEnd);
    };
  }, [applyCssTransform]);

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

    const newView = {
      freqMin: Math.round(newFreqMin),
      freqMax: Math.round(newFreqMax),
      timeStart: Math.round(newTimeStart * 100) / 100,
      duration: sv.duration,
    };
    applyCssTransform(newView);
    setView(newView);
  }, [dragging, data, applyCssTransform]);

  const handleMouseUp = useCallback(() => {
    setDragging(false);
    dragStart.current = null;
  }, []);

  const resetZoom = useCallback(() => {
    setView(DEFAULT_VIEW);
    setCssTransform('');
  }, []);

  const isZoomed = view.freqMin !== 0 || view.freqMax !== 24000 || view.timeStart !== 0 || view.duration !== 0;

  // Compute zoom level and viewport info
  const currentTimeRange = view.duration > 0 ? view.duration : fullDuration;
  const zoomPercent = fullDuration > 0 ? Math.round((fullDuration / currentTimeRange) * 100) : 100;
  const viewTimeEnd = view.timeStart + currentTimeRange;

  // Minimap viewport fraction
  const minimapLeft = fullDuration > 0 ? (view.timeStart / fullDuration) * 100 : 0;
  const minimapWidth = fullDuration > 0 ? (currentTimeRange / fullDuration) * 100 : 100;

  return (
    <Card label="Spectrogram display">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">SPECTROGRAM</span>
        <div className="flex items-center gap-3">
          {isZoomed && (
            <span className="text-zinc-500 text-xs font-data">
              {view.timeStart.toFixed(1)}s – {viewTimeEnd.toFixed(1)}s
              {fullDuration > 0 && <span className="text-zinc-600"> / {fullDuration.toFixed(1)}s</span>}
              <span className="text-zinc-600 ml-1.5">{zoomPercent}%</span>
            </span>
          )}
          {isZoomed && (
            <button
              onClick={resetZoom}
              className="text-zinc-500 hover:text-zinc-300 text-xs min-h-[44px] sm:min-h-0"
              aria-label="Reset spectrogram zoom"
            >
              Reset
            </button>
          )}
          {loading && <span className="text-zinc-400 text-xs" aria-live="polite">Loading...</span>}
          <span className="text-zinc-600 text-xs hidden sm:inline">Scroll to zoom, drag to pan</span>
        </div>
      </div>

      {/* Minimap overview */}
      {isZoomed && fullDuration > 0 && (
        <div className="relative h-1.5 bg-zinc-900 rounded-full mb-2 overflow-hidden">
          <div
            className="absolute top-0 h-full bg-zinc-600 rounded-full"
            style={{ left: `${minimapLeft}%`, width: `${Math.max(minimapWidth, 1)}%` }}
          />
        </div>
      )}

      <div className="flex gap-2">
        <div
          ref={containerRef}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          className="relative overflow-hidden flex-1"
          style={{ cursor: dragging ? 'grabbing' : 'grab' }}
        >
          <canvas
            ref={canvasRef}
            className="w-full h-48 rounded"
            style={{
              imageRendering: 'pixelated',
              transform: cssTransform || undefined,
              transformOrigin: '0 0',
              transition: cssTransform ? 'none' : undefined,
            }}
            aria-label="Spectrogram frequency visualization"
            role="img"
          />
        </div>
        {/* dB colorbar */}
        {data && (
          <div className="flex flex-col items-center justify-between h-48 shrink-0">
            <span className="text-zinc-500 text-[0.6rem] font-data">{dbRange[1].toFixed(0)}</span>
            <div
              className="w-3 flex-1 my-0.5 rounded-sm"
              style={{
                background: `linear-gradient(to bottom, rgb(253,231,37), rgb(115,208,86), rgb(35,137,142), rgb(65,68,135), rgb(68,1,84))`,
              }}
            />
            <span className="text-zinc-500 text-[0.6rem] font-data">{dbRange[0].toFixed(0)}</span>
            <span className="text-zinc-600 text-[0.55rem]">dB</span>
          </div>
        )}
      </div>
      {data && (
        <div className="flex justify-between text-zinc-500 text-xs mt-1 font-data">
          <span>{data.time_start.toFixed(1)}s</span>
          <span>{(data.freq_min / 1000).toFixed(1)}kHz – {(data.freq_max / 1000).toFixed(1)}kHz</span>
          <span>{data.time_end.toFixed(1)}s</span>
        </div>
      )}
    </Card>
  );
}
