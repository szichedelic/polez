import { useState } from 'react';
import type { FileInfo } from '../api/client';
import { loadFile } from '../api/client';

interface Props {
  fileInfo: FileInfo | null;
  onFileLoaded: (info: FileInfo) => void;
}

export function FileHeader({ fileInfo, onFileLoaded }: Props) {
  const [path, setPath] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleLoad = async () => {
    if (!path.trim()) return;
    setLoading(true);
    setError(null);
    try {
      const info = await loadFile(path.trim());
      onFileLoaded(info);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const formatDuration = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  return (
    <div className="flex items-center gap-4 bg-zinc-900 border-b border-zinc-700 px-4 py-3">
      <span className="text-purple-400 font-bold text-lg tracking-wider">POLEZ</span>
      <div className="flex items-center gap-2 flex-1">
        <input
          type="text"
          value={path}
          onChange={(e) => setPath(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && handleLoad()}
          placeholder="Enter file path..."
          className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-3 py-1.5 flex-1 text-sm focus:outline-none focus:border-purple-500"
        />
        <button
          onClick={handleLoad}
          disabled={loading}
          className="bg-purple-600 hover:bg-purple-500 disabled:opacity-50 text-white px-4 py-1.5 rounded text-sm font-medium"
        >
          {loading ? 'Loading...' : 'Open File'}
        </button>
      </div>
      {error && <span className="text-red-400 text-sm">{error}</span>}
      {fileInfo && (
        <div className="flex items-center gap-4 text-zinc-400 text-sm">
          <span className="text-zinc-200">{fileInfo.file_path.split('/').pop()}</span>
          <span>{fileInfo.format.toUpperCase()}</span>
          <span>{fileInfo.sample_rate / 1000}kHz</span>
          <span>{fileInfo.channels}ch</span>
          <span>{formatDuration(fileInfo.duration_secs)}</span>
        </div>
      )}
    </div>
  );
}
