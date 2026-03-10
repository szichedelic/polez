import { useState, useEffect, useRef, useCallback } from 'react';
import type { FileInfo } from './api/client';
import { getSession, uploadFile } from './api/client';
import { FileHeader } from './components/FileHeader';
import { Waveform } from './components/Waveform';
import { Spectrogram } from './components/Spectrogram';
import { DetectionPanel } from './components/DetectionPanel';
import { BitPlaneViewer } from './components/BitPlaneViewer';
import { CleanPanel } from './components/CleanPanel';
import { AudioPlayer } from './components/AudioPlayer';
import { ComparisonTimeline } from './components/ComparisonTimeline';
import { MetadataViewer } from './components/MetadataViewer';
import { BatchPanel } from './components/BatchPanel';
import { ErrorBoundary } from './components/ErrorBoundary';
import { useKeyboardShortcuts, SHORTCUT_LIST } from './hooks/useKeyboardShortcuts';

function App() {
  const [fileInfo, setFileInfo] = useState<FileInfo | null>(null);
  const [hasCleaned, setHasCleaned] = useState(false);
  const [showShortcuts, setShowShortcuts] = useState(false);
  const uploadInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    getSession().then((session) => {
      if (session.file_loaded && session.file_info) {
        setFileInfo(session.file_info);
        setHasCleaned(session.has_cleaned);
      }
    }).catch(() => {});
  }, []);

  const handleFileLoaded = (info: FileInfo) => {
    setFileInfo(info);
    setHasCleaned(false);
  };

  const togglePlay = useCallback(() => {
    const audio = document.querySelector('audio') as HTMLAudioElement | null;
    if (!audio) return;
    if (audio.paused) {
      audio.play().catch(() => {});
    } else {
      audio.pause();
    }
  }, []);

  const seekBy = useCallback((delta: number) => {
    const audio = document.querySelector('audio') as HTMLAudioElement | null;
    if (audio) audio.currentTime = Math.max(0, audio.currentTime + delta);
  }, []);

  useKeyboardShortcuts({
    onTogglePlay: togglePlay,
    onUpload: () => uploadInputRef.current?.click(),
    onDetect: () => (document.querySelector('[data-action="detect"]') as HTMLElement)?.click(),
    onClean: () => (document.querySelector('[data-action="clean"]') as HTMLElement)?.click(),
    onSave: () => (document.querySelector('[data-action="save"]') as HTMLElement)?.click(),
    onSeekBack: () => seekBy(-5),
    onSeekForward: () => seekBy(5),
    onShowHelp: () => setShowShortcuts(v => !v),
  });

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <input
        ref={uploadInputRef}
        type="file"
        accept="audio/*"
        className="hidden"
        aria-label="Upload audio file"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) {
            uploadFile(file).then(handleFileLoaded).catch(() => {});
          }
        }}
      />

      {showShortcuts && (
        <div
          className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4"
          role="dialog"
          aria-modal="true"
          aria-label="Keyboard shortcuts"
          onClick={() => setShowShortcuts(false)}
          onKeyDown={(e) => { if (e.key === 'Escape') setShowShortcuts(false); }}
        >
          <div
            className="bg-zinc-800 border border-zinc-600 rounded-lg p-6 max-w-sm"
            onClick={(e) => e.stopPropagation()}
          >
            <h2 className="text-zinc-100 font-bold text-lg mb-4">Keyboard Shortcuts</h2>
            <div className="space-y-2" role="list">
              {SHORTCUT_LIST.map((s) => (
                <div key={s.keys} className="flex justify-between gap-6" role="listitem">
                  <kbd className="text-purple-400 font-mono text-sm bg-zinc-700 px-2 py-0.5 rounded">
                    {s.keys}
                  </kbd>
                  <span className="text-zinc-400 text-sm">{s.description}</span>
                </div>
              ))}
            </div>
            <button
              onClick={() => setShowShortcuts(false)}
              className="mt-4 w-full text-center text-zinc-500 text-sm hover:text-zinc-300"
              aria-label="Close keyboard shortcuts dialog"
            >
              Press ? or click to close
            </button>
          </div>
        </div>
      )}

      <header>
        <ErrorBoundary section="Upload">
          <FileHeader fileInfo={fileInfo} onFileLoaded={handleFileLoaded} />
        </ErrorBoundary>
      </header>

      <main className="p-4 space-y-4">
        <ErrorBoundary section="Audio Player">
          <AudioPlayer fileLoaded={!!fileInfo} hasCleaned={hasCleaned} />
        </ErrorBoundary>

        <ErrorBoundary section="Waveform">
          <Waveform fileLoaded={!!fileInfo} />
        </ErrorBoundary>

        <ErrorBoundary section="Spectrogram">
          <Spectrogram fileLoaded={!!fileInfo} />
        </ErrorBoundary>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <ErrorBoundary section="Detection">
            <DetectionPanel fileLoaded={!!fileInfo} />
          </ErrorBoundary>
          <ErrorBoundary section="Bit Plane">
            <div className="hidden md:block">
              <BitPlaneViewer fileLoaded={!!fileInfo} />
            </div>
          </ErrorBoundary>
        </div>

        <ErrorBoundary section="Metadata">
          <MetadataViewer fileLoaded={!!fileInfo} hasCleaned={hasCleaned} />
        </ErrorBoundary>

        <ErrorBoundary section="Cleaning">
          <CleanPanel fileLoaded={!!fileInfo} onCleaned={() => setHasCleaned(true)} />
        </ErrorBoundary>

        <ErrorBoundary section="Comparison">
          <ComparisonTimeline fileLoaded={!!fileInfo} hasCleaned={hasCleaned} />
        </ErrorBoundary>

        <ErrorBoundary section="Batch Processing">
          <BatchPanel />
        </ErrorBoundary>
      </main>
    </div>
  );
}

export default App;
