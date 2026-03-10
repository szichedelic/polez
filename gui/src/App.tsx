import { useState } from 'react';
import type { FileInfo } from './api/client';
import { FileHeader } from './components/FileHeader';
import { Waveform } from './components/Waveform';
import { Spectrogram } from './components/Spectrogram';
import { DetectionPanel } from './components/DetectionPanel';
import { BitPlaneViewer } from './components/BitPlaneViewer';
import { CleanPanel } from './components/CleanPanel';
import { AudioPlayer } from './components/AudioPlayer';

function App() {
  const [fileInfo, setFileInfo] = useState<FileInfo | null>(null);
  const [hasCleaned, setHasCleaned] = useState(false);

  const handleFileLoaded = (info: FileInfo) => {
    setFileInfo(info);
    setHasCleaned(false);
  };

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <FileHeader fileInfo={fileInfo} onFileLoaded={handleFileLoaded} />

      <div className="p-4 space-y-4">
        <AudioPlayer fileLoaded={!!fileInfo} hasCleaned={hasCleaned} />
        <Waveform fileLoaded={!!fileInfo} />
        <Spectrogram fileLoaded={!!fileInfo} />

        <div className="grid grid-cols-2 gap-4">
          <DetectionPanel fileLoaded={!!fileInfo} />
          <BitPlaneViewer fileLoaded={!!fileInfo} />
        </div>

        <CleanPanel fileLoaded={!!fileInfo} onCleaned={() => setHasCleaned(true)} />
      </div>
    </div>
  );
}

export default App;
