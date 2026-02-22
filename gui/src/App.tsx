import { useState } from 'react';
import type { FileInfo } from './api/client';
import { FileHeader } from './components/FileHeader';
import { Waveform } from './components/Waveform';
import { Spectrogram } from './components/Spectrogram';
import { DetectionPanel } from './components/DetectionPanel';
import { BitPlaneViewer } from './components/BitPlaneViewer';

function App() {
  const [fileInfo, setFileInfo] = useState<FileInfo | null>(null);

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <FileHeader fileInfo={fileInfo} onFileLoaded={setFileInfo} />

      <div className="p-4 space-y-4">
        <Waveform fileLoaded={!!fileInfo} />
        <Spectrogram fileLoaded={!!fileInfo} />

        <div className="grid grid-cols-2 gap-4">
          <DetectionPanel fileLoaded={!!fileInfo} />
          <BitPlaneViewer fileLoaded={!!fileInfo} />
        </div>
      </div>
    </div>
  );
}

export default App;
