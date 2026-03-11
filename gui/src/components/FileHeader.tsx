import { useState, useRef, useCallback } from 'react';
import type { FileInfo } from '../api/client';
import { uploadFile, MAX_UPLOAD_BYTES } from '../api/client';

interface Props {
  fileInfo: FileInfo | null;
  onFileLoaded: (info: FileInfo) => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function FileHeader({ fileInfo, onFileLoaded }: Props) {
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const handleFile = useCallback(async (file: File) => {
    setError(null);

    if (file.size > MAX_UPLOAD_BYTES) {
      setError(`File too large (${formatBytes(file.size)}). Maximum is ${formatBytes(MAX_UPLOAD_BYTES)}.`);
      return;
    }

    setLoading(true);
    setProgress(0);
    try {
      const info = await uploadFile(file, (pct) => setProgress(pct));
      onFileLoaded(info);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
      setProgress(0);
    }
  }, [onFileLoaded]);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) handleFile(file);
  }, [handleFile]);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(true);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDragOver(false);
  }, []);

  const handlePickerChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) handleFile(file);
  }, [handleFile]);

  const formatDuration = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  if (fileInfo && !loading) {
    return (
      <nav className="flex flex-wrap items-center gap-2 sm:gap-4 bg-zinc-900 border-b border-zinc-700 px-4 py-3" aria-label="File information">
        <span className="text-zinc-200 font-bold text-lg tracking-wider">POLEZ</span>
        <div className="flex flex-wrap items-center gap-2 sm:gap-4 text-zinc-400 text-sm flex-1 min-w-0">
          <span className="text-zinc-200 font-medium truncate">{fileInfo.file_path.split('/').pop()}</span>
          <span>{fileInfo.format.toUpperCase()}</span>
          <span>{fileInfo.sample_rate / 1000}kHz</span>
          <span>{fileInfo.channels}ch</span>
          <span>{formatDuration(fileInfo.duration_secs)}</span>
        </div>
        <button
          onClick={() => inputRef.current?.click()}
          className="text-zinc-400 hover:text-zinc-200 text-sm min-h-[44px] min-w-[44px] flex items-center justify-center"
          aria-label="Change audio file"
        >
          Change file
        </button>
        <input
          ref={inputRef}
          type="file"
          accept="audio/*"
          onChange={handlePickerChange}
          className="hidden"
          aria-label="Choose audio file"
        />
      </nav>
    );
  }

  return (
    <div
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      role="region"
      aria-label="File upload area"
      className={`flex flex-col items-center justify-center gap-3 border-2 border-dashed rounded-lg mx-4 my-4 py-12 transition-colors ${
        dragOver
          ? 'border-zinc-500 bg-zinc-800/50'
          : 'border-zinc-600 bg-zinc-900 hover:border-zinc-500'
      }`}
    >
      <div className="text-zinc-200 font-bold text-2xl tracking-widest mb-2">POLEZ</div>
      <div className="text-zinc-500 text-xs mb-4">Audio Forensics & Sanitization Engine</div>
      {loading ? (
        <div className="w-64 space-y-2">
          <div className="text-zinc-400 text-sm text-center">
            Uploading... {progress}%
          </div>
          <div className="w-full bg-zinc-700 rounded-full h-2" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100} aria-label="Upload progress">
            <div
              className="bg-zinc-400 h-2 rounded-full transition-all duration-200"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      ) : (
        <>
          <div className="text-zinc-400 text-sm">
            Drag & drop an audio file here
          </div>
          <div className="text-zinc-500 text-xs">or</div>
          <button
            onClick={() => inputRef.current?.click()}
            className="bg-zinc-700 hover:bg-zinc-600 text-zinc-200 px-4 py-2.5 sm:py-1.5 rounded text-sm font-medium min-h-[44px]"
            aria-label="Choose audio file to upload"
          >
            Choose File
          </button>
          <div className="text-zinc-600 text-xs mt-1">
            Max {formatBytes(MAX_UPLOAD_BYTES)}
          </div>
          <input
            ref={inputRef}
            type="file"
            accept="audio/*"
            onChange={handlePickerChange}
            className="hidden"
            aria-label="Choose audio file"
          />
        </>
      )}
      {error && <span className="text-red-400 text-sm" role="alert">{error}</span>}
    </div>
  );
}
