import { useEffect, useRef } from 'react';
import WaveSurfer from 'wavesurfer.js';
import { getAudioUrl } from '../api/client';

interface Props {
  fileLoaded: boolean;
}

export function Waveform({ fileLoaded }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WaveSurfer | null>(null);

  useEffect(() => {
    if (!fileLoaded || !containerRef.current) return;

    if (wsRef.current) {
      wsRef.current.destroy();
    }

    const ws = WaveSurfer.create({
      container: containerRef.current,
      waveColor: '#a1a1aa',
      progressColor: '#71717a',
      cursorColor: '#e4e4e7',
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: 128,
      normalize: true,
    });

    ws.load(getAudioUrl());
    wsRef.current = ws;

    return () => {
      ws.destroy();
    };
  }, [fileLoaded]);

  return (
    <section className="bg-zinc-900 border border-zinc-700 rounded p-4" aria-label="Waveform display">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <span className="text-zinc-400 text-sm font-medium">WAVEFORM</span>
        <div className="flex gap-2">
          <button
            onClick={() => wsRef.current?.playPause()}
            className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-2 sm:py-1 rounded text-sm min-h-[44px] sm:min-h-0"
            aria-label="Play or pause waveform"
          >
            Play/Pause
          </button>
          <button
            onClick={() => wsRef.current?.stop()}
            className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-2 sm:py-1 rounded text-sm min-h-[44px] sm:min-h-0"
            aria-label="Stop waveform playback"
          >
            Stop
          </button>
        </div>
      </div>
      <div ref={containerRef} aria-label="Audio waveform visualization" role="img" />
    </section>
  );
}
