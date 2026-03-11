import { useState, useRef, useEffect, useCallback } from 'react';
import WaveSurfer from 'wavesurfer.js';
import { getAudioUrl, getCleanedAudioUrl } from '../api/client';
import type { FileInfo } from '../api/client';
import { Card } from './Card';

interface Props {
  fileInfo: FileInfo | null;
  hasCleaned: boolean;
}

function formatTime(secs: number): string {
  if (!isFinite(secs) || secs < 0) return '0:00';
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function TransportBar({ fileInfo, hasCleaned }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WaveSurfer | null>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [source, setSource] = useState<'original' | 'cleaned'>('original');

  const audioUrl = source === 'cleaned' && hasCleaned ? getCleanedAudioUrl() : getAudioUrl();

  // Reset on file change
  useEffect(() => {
    setPlaying(false);
    setCurrentTime(0);
    setDuration(0);
  }, [fileInfo]);

  // Load WaveSurfer waveform
  useEffect(() => {
    if (!fileInfo || !containerRef.current) return;

    if (wsRef.current) {
      wsRef.current.destroy();
    }

    const ws = WaveSurfer.create({
      container: containerRef.current,
      waveColor: '#a1a1aa',
      progressColor: 'rgba(161, 161, 170, 0.3)',
      cursorColor: 'rgba(250, 250, 250, 0.7)',
      cursorWidth: 1.5,
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: 96,
      normalize: true,
      interact: false,
      backend: 'WebAudio',
    });

    ws.load(getAudioUrl());
    wsRef.current = ws;

    return () => {
      ws.destroy();
      wsRef.current = null;
    };
  }, [fileInfo]);

  // Update waveform progress from audio element (throttled to rAF)
  const rafRef = useRef<number>(0);
  useEffect(() => {
    const ws = wsRef.current;
    if (!ws || duration <= 0) return;
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(() => {
      const progress = currentTime / duration;
      ws.seekTo(Math.min(1, Math.max(0, progress)));
    });
    return () => cancelAnimationFrame(rafRef.current);
  }, [currentTime, duration]);

  // Source switching
  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const wasPlaying = playing;
    audio.pause();
    audio.src = audioUrl;
    audio.load();
    setCurrentTime(0);
    setPlaying(false);

    if (wasPlaying) {
      audio.addEventListener('canplay', () => audio.play().then(() => setPlaying(true)).catch(() => {}), { once: true });
    }
  }, [source]);

  const onTimeUpdate = useCallback(() => {
    if (audioRef.current) {
      setCurrentTime(audioRef.current.currentTime);
    }
  }, []);

  const onLoadedMetadata = useCallback(() => {
    if (audioRef.current) {
      setDuration(audioRef.current.duration);
    }
  }, []);

  const onEnded = useCallback(() => {
    setPlaying(false);
  }, []);

  const togglePlay = () => {
    const audio = audioRef.current;
    if (!audio) return;
    if (playing) {
      audio.pause();
      setPlaying(false);
    } else {
      audio.play().then(() => setPlaying(true)).catch(() => {});
    }
  };

  const handleWaveformClick = useCallback((e: React.MouseEvent<HTMLDivElement>) => {
    const container = containerRef.current;
    const audio = audioRef.current;
    if (!container || !audio || !duration) return;

    const rect = container.getBoundingClientRect();
    const fraction = (e.clientX - rect.left) / rect.width;
    audio.currentTime = fraction * duration;
    setCurrentTime(audio.currentTime);
  }, [duration]);

  const handleVolume = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = parseFloat(e.target.value);
    setVolume(v);
    if (audioRef.current) audioRef.current.volume = v;
  };

  if (!fileInfo) return null;

  return (
    <Card label="Audio transport" variant="recessed">
      <audio
        ref={audioRef}
        src={audioUrl}
        onTimeUpdate={onTimeUpdate}
        onLoadedMetadata={onLoadedMetadata}
        onEnded={onEnded}
      />

      {/* Waveform */}
      <div
        ref={containerRef}
        onClick={handleWaveformClick}
        className="cursor-pointer mb-3 rounded"
        aria-label="Audio position"
        role="slider"
        tabIndex={0}
        aria-valuemin={0}
        aria-valuemax={Math.round(duration)}
        aria-valuenow={Math.round(currentTime)}
      />

      {/* Transport controls */}
      <div className="flex flex-wrap items-center gap-2 sm:gap-3">
        {/* Play/Pause */}
        <button
          onClick={togglePlay}
          className="w-9 h-9 sm:w-7 sm:h-7 flex items-center justify-center bg-zinc-800 hover:bg-zinc-700 rounded-full text-zinc-50 shrink-0"
          title={playing ? 'Pause' : 'Play'}
          aria-label={playing ? 'Pause audio' : 'Play audio'}
        >
          {playing ? (
            <svg width="12" height="12" viewBox="0 0 14 14" fill="currentColor">
              <rect x="2" y="1" width="4" height="12" rx="1" />
              <rect x="8" y="1" width="4" height="12" rx="1" />
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 14 14" fill="currentColor">
              <polygon points="3,1 13,7 3,13" />
            </svg>
          )}
        </button>

        {/* Time */}
        <span className="text-xs text-zinc-500 font-data shrink-0">
          {formatTime(currentTime)}
          <span className="text-zinc-700 mx-0.5">/</span>
          {formatTime(duration)}
        </span>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Volume */}
        <div className="hidden sm:flex items-center gap-1.5 shrink-0">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" className="text-zinc-600">
            <polygon points="1,5 4,5 7,2 7,12 4,9 1,9" />
            {volume > 0.5 && <path d="M9,3 Q13,7 9,11" fill="none" stroke="currentColor" strokeWidth="1.5" />}
            {volume > 0 && <path d="M9,5 Q11,7 9,9" fill="none" stroke="currentColor" strokeWidth="1.5" />}
          </svg>
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={volume}
            onChange={handleVolume}
            className="w-14 h-1 accent-zinc-500 cursor-pointer"
            aria-label={`Volume: ${Math.round(volume * 100)}%`}
          />
        </div>

        {/* Metadata */}
        <div className="hidden sm:flex items-center gap-2 text-zinc-700 text-[0.65rem] font-data shrink-0">
          <span>{fileInfo.format.toUpperCase()}</span>
          <span>{fileInfo.sample_rate / 1000}kHz</span>
          <span>{fileInfo.channels}ch</span>
        </div>

        {/* Source toggle */}
        {hasCleaned && (
          <div className="flex gap-1 shrink-0" role="group" aria-label="Audio source">
            <button
              onClick={() => setSource('original')}
              className={`px-2 py-1 rounded text-xs min-h-[44px] sm:min-h-0 ${source === 'original' ? 'bg-zinc-800 text-zinc-200' : 'text-zinc-600 hover:text-zinc-400'}`}
              aria-pressed={source === 'original'}
              aria-label="Play original audio"
            >
              Original
            </button>
            <button
              onClick={() => setSource('cleaned')}
              className={`px-2 py-1 rounded text-xs min-h-[44px] sm:min-h-0 ${source === 'cleaned' ? 'bg-zinc-800 text-zinc-200' : 'text-zinc-600 hover:text-zinc-400'}`}
              aria-pressed={source === 'cleaned'}
              aria-label="Play cleaned audio"
            >
              Cleaned
            </button>
          </div>
        )}
      </div>
    </Card>
  );
}
