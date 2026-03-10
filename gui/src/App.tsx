import { useState, useEffect } from 'react';
import type { FileInfo } from './api/client';
import { getSession } from './api/client';
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

function App() {
  const [fileInfo, setFileInfo] = useState<FileInfo | null>(null);
  const [hasCleaned, setHasCleaned] = useState(false);

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

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <ErrorBoundary section="Upload">
        <FileHeader fileInfo={fileInfo} onFileLoaded={handleFileLoaded} />
      </ErrorBoundary>

      <div className="p-4 space-y-4">
        <ErrorBoundary section="Audio Player">
          <AudioPlayer fileLoaded={!!fileInfo} hasCleaned={hasCleaned} />
        </ErrorBoundary>

        <ErrorBoundary section="Waveform">
          <Waveform fileLoaded={!!fileInfo} />
        </ErrorBoundary>

        <ErrorBoundary section="Spectrogram">
          <Spectrogram fileLoaded={!!fileInfo} />
        </ErrorBoundary>

        <div className="grid grid-cols-2 gap-4">
          <ErrorBoundary section="Detection">
            <DetectionPanel fileLoaded={!!fileInfo} />
          </ErrorBoundary>
          <ErrorBoundary section="Bit Plane">
            <BitPlaneViewer fileLoaded={!!fileInfo} />
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
      </div>
    </div>
  );
}

export default App;
