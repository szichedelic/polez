import { useEffect, useRef, useState, useCallback } from 'react';
import WaveSurfer from 'wavesurfer.js';
import { getAudioUrl, getCleanedAudioUrl } from '../api/client';

interface Props {
  fileLoaded: boolean;
  hasCleaned: boolean;
}

type ViewMode = 'side-by-side' | 'overlay';

export function ComparisonTimeline({ fileLoaded, hasCleaned }: Props) {
  const origRef = useRef<HTMLDivElement>(null);
  const cleanRef = useRef<HTMLDivElement>(null);
  const wsOrigRef = useRef<WaveSurfer | null>(null);
  const wsCleanRef = useRef<WaveSurfer | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>('side-by-side');
  const [playing, setPlaying] = useState(false);
  const [activeSource, setActiveSource] = useState<'original' | 'cleaned' | 'both'>('both');
  const syncingRef = useRef(false);

  const destroyInstances = useCallback(() => {
    wsOrigRef.current?.destroy();
    wsCleanRef.current?.destroy();
    wsOrigRef.current = null;
    wsCleanRef.current = null;
    setPlaying(false);
  }, []);

  useEffect(() => {
    if (!hasCleaned || !fileLoaded) {
      destroyInstances();
      return;
    }

    if (!origRef.current || !cleanRef.current) return;

    destroyInstances();

    const origWs = WaveSurfer.create({
      container: origRef.current,
      waveColor: '#a1a1aa',
      progressColor: '#71717a',
      cursorColor: '#e4e4e7',
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: 80,
      normalize: true,
      interact: true,
    });

    const cleanWs = WaveSurfer.create({
      container: cleanRef.current,
      waveColor: '#10b981',
      progressColor: '#059669',
      cursorColor: '#e4e4e7',
      barWidth: 2,
      barGap: 1,
      barRadius: 2,
      height: 80,
      normalize: true,
      interact: true,
    });

    origWs.load(getAudioUrl());
    cleanWs.load(getCleanedAudioUrl());

    const syncSeek = (source: WaveSurfer, target: WaveSurfer) => {
      source.on('seeking', (time: number) => {
        if (syncingRef.current) return;
        syncingRef.current = true;
        const progress = time / source.getDuration();
        target.seekTo(progress);
        syncingRef.current = false;
      });
    };

    syncSeek(origWs, cleanWs);
    syncSeek(cleanWs, origWs);

    origWs.on('finish', () => setPlaying(false));
    cleanWs.on('finish', () => setPlaying(false));

    wsOrigRef.current = origWs;
    wsCleanRef.current = cleanWs;

    return destroyInstances;
  }, [hasCleaned, fileLoaded, destroyInstances]);

  const togglePlayPause = () => {
    const orig = wsOrigRef.current;
    const clean = wsCleanRef.current;
    if (!orig || !clean) return;

    if (playing) {
      orig.pause();
      clean.pause();
      setPlaying(false);
    } else {
      if (activeSource === 'both' || activeSource === 'original') orig.play();
      if (activeSource === 'both' || activeSource === 'cleaned') clean.play();
      setPlaying(true);
    }
  };

  const stop = () => {
    wsOrigRef.current?.stop();
    wsCleanRef.current?.stop();
    setPlaying(false);
  };

  const handleSourceToggle = (src: 'original' | 'cleaned' | 'both') => {
    const orig = wsOrigRef.current;
    const clean = wsCleanRef.current;
    if (!orig || !clean) return;

    if (playing) {
      if (src === 'both' || src === 'original') {
        if (orig.isPlaying()) { /* already playing */ } else orig.play();
      } else {
        orig.pause();
      }
      if (src === 'both' || src === 'cleaned') {
        if (clean.isPlaying()) { /* already playing */ } else clean.play();
      } else {
        clean.pause();
      }
    }
    setActiveSource(src);
  };

  if (!fileLoaded || !hasCleaned) return null;

  return (
    <section className="bg-zinc-900 border border-zinc-700 rounded p-4" aria-label="A/B audio comparison">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <span className="text-zinc-400 text-sm font-medium">A/B COMPARISON</span>

        <div className="flex flex-wrap items-center gap-2 sm:gap-3">
          <div className="flex gap-1" role="group" aria-label="Audio source selection">
            {(['both', 'original', 'cleaned'] as const).map((src) => (
              <button
                key={src}
                onClick={() => handleSourceToggle(src)}
                aria-pressed={activeSource === src}
                aria-label={`Play ${src} audio`}
                className={`px-2 py-1.5 sm:py-0.5 rounded text-xs capitalize min-h-[44px] sm:min-h-0 ${
                  activeSource === src
                    ? src === 'cleaned'
                      ? 'bg-emerald-700 text-emerald-100'
                      : src === 'original'
                        ? 'bg-zinc-600 text-zinc-100'
                        : 'bg-zinc-600 text-zinc-100'
                    : 'text-zinc-500 hover:text-zinc-300'
                }`}
              >
                {src}
              </button>
            ))}
          </div>

          <div className="hidden sm:block w-px h-4 bg-zinc-700" />

          <div className="flex gap-1" role="group" aria-label="View mode">
            <button
              onClick={() => setViewMode('side-by-side')}
              className={`px-2 py-1.5 sm:py-0.5 rounded text-xs min-h-[44px] sm:min-h-0 ${viewMode === 'side-by-side' ? 'bg-zinc-700 text-zinc-200' : 'text-zinc-500 hover:text-zinc-300'}`}
              aria-pressed={viewMode === 'side-by-side'}
              aria-label="Split view"
            >
              Split
            </button>
            <button
              onClick={() => setViewMode('overlay')}
              className={`px-2 py-1.5 sm:py-0.5 rounded text-xs min-h-[44px] sm:min-h-0 ${viewMode === 'overlay' ? 'bg-zinc-700 text-zinc-200' : 'text-zinc-500 hover:text-zinc-300'}`}
              aria-pressed={viewMode === 'overlay'}
              aria-label="Overlay view"
            >
              Overlay
            </button>
          </div>

          <div className="hidden sm:block w-px h-4 bg-zinc-700" />

          <div className="flex gap-2">
            <button
              onClick={togglePlayPause}
              className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-2 sm:py-1 rounded text-sm min-h-[44px] sm:min-h-0"
              aria-label={playing ? 'Pause comparison playback' : 'Play comparison'}
            >
              {playing ? 'Pause' : 'Play'}
            </button>
            <button
              onClick={stop}
              className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-3 py-2 sm:py-1 rounded text-sm min-h-[44px] sm:min-h-0"
              aria-label="Stop comparison playback"
            >
              Stop
            </button>
          </div>
        </div>
      </div>

      {viewMode === 'side-by-side' ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          <div>
            <div className="flex items-center gap-2 mb-1">
              <div className="w-2 h-2 rounded-full bg-zinc-400" />
              <span className="text-xs text-zinc-400">Original</span>
            </div>
            <div
              ref={origRef}
              className={activeSource === 'cleaned' ? 'opacity-30' : ''}
            />
          </div>
          <div>
            <div className="flex items-center gap-2 mb-1">
              <div className="w-2 h-2 rounded-full bg-emerald-500" />
              <span className="text-xs text-zinc-400">Cleaned</span>
            </div>
            <div
              ref={cleanRef}
              className={activeSource === 'original' ? 'opacity-30' : ''}
            />
          </div>
        </div>
      ) : (
        <div className="relative">
          <div className="flex items-center gap-4 mb-1">
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-zinc-400" />
              <span className="text-xs text-zinc-400">Original</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-2 h-2 rounded-full bg-emerald-500" />
              <span className="text-xs text-zinc-400">Cleaned</span>
            </div>
          </div>
          <div
            ref={origRef}
            className={activeSource === 'cleaned' ? 'opacity-20' : 'opacity-60'}
          />
          <div
            ref={cleanRef}
            className={`-mt-[80px] ${activeSource === 'original' ? 'opacity-20' : 'opacity-60'}`}
          />
        </div>
      )}
    </section>
  );
}
