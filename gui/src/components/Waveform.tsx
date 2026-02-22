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
      waveColor: '#a855f7',
      progressColor: '#7c3aed',
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
    <div className="bg-zinc-900 border border-zinc-700 rounded p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-zinc-400 text-sm font-medium">WAVEFORM</span>
        <div className="flex gap-2">
          <button
            onClick={() => wsRef.current?.playPause()}
            className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-1 rounded text-sm"
          >
            Play/Pause
          </button>
          <button
            onClick={() => wsRef.current?.stop()}
            className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-1 rounded text-sm"
          >
            Stop
          </button>
        </div>
      </div>
      <div ref={containerRef} />
    </div>
  );
}
