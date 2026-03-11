import { useEffect, useRef } from 'react';
import WaveSurfer from 'wavesurfer.js';
import { getAudioUrl } from '../api/client';
import { Card } from './Card';
import { Button } from './Button';

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
    <Card label="Waveform display">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-2">
        <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">WAVEFORM</span>
        <div className="flex gap-2">
          <Button onClick={() => wsRef.current?.playPause()} aria-label="Play or pause waveform">
            Play/Pause
          </Button>
          <Button onClick={() => wsRef.current?.stop()} aria-label="Stop waveform playback">
            Stop
          </Button>
        </div>
      </div>
      <div ref={containerRef} aria-label="Audio waveform visualization" role="img" />
    </Card>
  );
}
