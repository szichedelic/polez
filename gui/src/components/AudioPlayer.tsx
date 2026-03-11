import { useState, useRef, useEffect, useCallback } from 'react';
import { getAudioUrl, getCleanedAudioUrl } from '../api/client';
import { Card } from './Card';

interface Props {
  fileLoaded: boolean;
  hasCleaned: boolean;
}

function formatTime(secs: number): string {
  if (!isFinite(secs) || secs < 0) return '0:00';
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function AudioPlayer({ fileLoaded, hasCleaned }: Props) {
  const audioRef = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [volume, setVolume] = useState(1);
  const [source, setSource] = useState<'original' | 'cleaned'>('original');
  const [seeking, setSeeking] = useState(false);

  const audioUrl = source === 'cleaned' && hasCleaned ? getCleanedAudioUrl() : getAudioUrl();

  useEffect(() => {
    setPlaying(false);
    setCurrentTime(0);
    setDuration(0);
  }, [fileLoaded]);

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
    if (!seeking && audioRef.current) {
      setCurrentTime(audioRef.current.currentTime);
    }
  }, [seeking]);

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

  const handleSeek = (e: React.ChangeEvent<HTMLInputElement>) => {
    const t = parseFloat(e.target.value);
    setCurrentTime(t);
  };

  const handleSeekStart = () => setSeeking(true);

  const handleSeekEnd = (_e: React.MouseEvent<HTMLInputElement> | React.TouchEvent<HTMLInputElement>) => {
    setSeeking(false);
    if (audioRef.current) {
      audioRef.current.currentTime = currentTime;
    }
  };

  const handleVolume = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = parseFloat(e.target.value);
    setVolume(v);
    if (audioRef.current) audioRef.current.volume = v;
  };

  const progress = duration > 0 ? (currentTime / duration) * 100 : 0;

  if (!fileLoaded) return null;

  return (
    <Card label="Audio player" padding="sm">
      <audio
        ref={audioRef}
        src={audioUrl}
        onTimeUpdate={onTimeUpdate}
        onLoadedMetadata={onLoadedMetadata}
        onEnded={onEnded}
      />

      <div className="flex flex-wrap items-center gap-2 sm:gap-3">
        <button
          onClick={togglePlay}
          className="w-11 h-11 sm:w-8 sm:h-8 flex items-center justify-center bg-zinc-800 hover:bg-zinc-700 rounded-full text-zinc-200 shrink-0"
          title={playing ? 'Pause' : 'Play'}
          aria-label={playing ? 'Pause audio' : 'Play audio'}
        >
          {playing ? (
            <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
              <rect x="2" y="1" width="4" height="12" rx="1" />
              <rect x="8" y="1" width="4" height="12" rx="1" />
            </svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
              <polygon points="3,1 13,7 3,13" />
            </svg>
          )}
        </button>

        <span className="text-xs text-zinc-400 w-10 text-right shrink-0 font-data">
          {formatTime(currentTime)}
        </span>

        <div className="flex-1 relative">
          <div className="h-1.5 bg-zinc-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-emerald-500 rounded-full"
              style={{ width: `${progress}%` }}
            />
          </div>
          <input
            type="range"
            min={0}
            max={duration || 0}
            step={0.1}
            value={currentTime}
            onChange={handleSeek}
            onMouseDown={handleSeekStart}
            onMouseUp={handleSeekEnd}
            onTouchStart={handleSeekStart}
            onTouchEnd={handleSeekEnd}
            className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
            aria-label={`Seek position: ${formatTime(currentTime)} of ${formatTime(duration)}`}
          />
        </div>

        <span className="text-xs text-zinc-400 w-10 shrink-0 font-data">
          {formatTime(duration)}
        </span>

        <div className="hidden sm:flex items-center gap-1.5 shrink-0">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" className="text-zinc-500">
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
            className="w-16 h-1 accent-emerald-500 cursor-pointer"
            aria-label={`Volume: ${Math.round(volume * 100)}%`}
          />
        </div>

        {hasCleaned && (
          <div className="flex gap-1 shrink-0 ml-1" role="group" aria-label="Audio source">
            <button
              onClick={() => setSource('original')}
              className={`px-2 py-1.5 sm:py-0.5 rounded text-xs min-h-[44px] sm:min-h-0 ${source === 'original' ? 'bg-zinc-700 text-zinc-200' : 'text-zinc-500 hover:text-zinc-300'}`}
              aria-pressed={source === 'original'}
              aria-label="Play original audio"
            >
              Original
            </button>
            <button
              onClick={() => setSource('cleaned')}
              className={`px-2 py-1.5 sm:py-0.5 rounded text-xs min-h-[44px] sm:min-h-0 ${source === 'cleaned' ? 'bg-emerald-700 text-emerald-100' : 'text-zinc-500 hover:text-zinc-300'}`}
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
