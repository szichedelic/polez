import { useEffect, useRef, useState } from 'react';
import { getSpectrogram, type SpectrogramData } from '../api/client';

interface Props {
  fileLoaded: boolean;
}

export function Spectrogram({ fileLoaded }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [data, setData] = useState<SpectrogramData | null>(null);
  const [loading, setLoading] = useState(false);
  const [freqMax, setFreqMax] = useState(24000);

  useEffect(() => {
    if (!fileLoaded) return;
    fetchSpectrogram();
  }, [fileLoaded, freqMax]);

  const fetchSpectrogram = async () => {
    setLoading(true);
    try {
      const d = await getSpectrogram({ freq_max: freqMax });
      setData(d);
    } finally {
      setLoading(false);
    }
  };

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

  return (
    <div className="bg-zinc-900 border border-zinc-700 rounded p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-zinc-400 text-sm font-medium">SPECTROGRAM</span>
        <div className="flex items-center gap-2">
          <label className="text-zinc-500 text-xs">Max Freq:</label>
          <select
            value={freqMax}
            onChange={(e) => setFreqMax(Number(e.target.value))}
            className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-2 py-1 text-xs"
          >
            <option value={8000}>8 kHz</option>
            <option value={16000}>16 kHz</option>
            <option value={22050}>22 kHz</option>
            <option value={24000}>24 kHz</option>
          </select>
          {loading && <span className="text-purple-400 text-xs">Loading...</span>}
        </div>
      </div>
      <canvas
        ref={canvasRef}
        className="w-full h-48 rounded"
        style={{ imageRendering: 'pixelated' }}
      />
      {data && (
        <div className="flex justify-between text-zinc-500 text-xs mt-1">
          <span>{data.time_start.toFixed(1)}s</span>
          <span>{data.freq_min / 1000}kHz - {data.freq_max / 1000}kHz</span>
          <span>{data.time_end.toFixed(1)}s</span>
        </div>
      )}
    </div>
  );
}
