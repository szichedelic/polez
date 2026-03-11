import { useState, useRef, useCallback } from 'react';
import type { FileInfo } from '../api/client';
import { uploadFile, MAX_UPLOAD_BYTES } from '../api/client';
import { Button } from './Button';

interface Props {
  fileInfo: FileInfo | null;
  onFileLoaded: (info: FileInfo) => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const FORMATS = ['WAV', 'MP3', 'FLAC', 'OGG'];

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

  // ── Loaded state: compact horizontal bar ──
  if (fileInfo && !loading) {
    return (
      <nav
        className="flex flex-wrap items-center gap-3 sm:gap-4 bg-zinc-900 border-b border-zinc-800 px-4 py-2.5"
        aria-label="File information"
      >
        <span className="font-heading text-zinc-50 font-semibold text-sm tracking-tight">POLEZ</span>
        <div className="w-px h-4 bg-zinc-800 hidden sm:block" />
        <span className="text-zinc-200 text-sm font-medium truncate min-w-0">{fileInfo.file_path.split('/').pop()}</span>
        <div className="flex items-center gap-3 text-zinc-500 text-xs font-data ml-auto">
          <span>{fileInfo.format.toUpperCase()}</span>
          <span>{fileInfo.sample_rate / 1000}kHz</span>
          <span>{fileInfo.channels}ch</span>
          <span>{formatDuration(fileInfo.duration_secs)}</span>
          <span>{formatBytes(fileInfo.duration_secs * fileInfo.sample_rate * fileInfo.channels * 2)}</span>
        </div>
        <Button variant="ghost" onClick={() => inputRef.current?.click()} aria-label="Change audio file">
          Change
        </Button>
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

  // ── Empty state: branded upload hero ──
  return (
    <div
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      role="region"
      aria-label="File upload area"
      className="flex flex-col items-center justify-center bg-zinc-950 px-4 py-16"
    >
      <h1 className="font-heading text-zinc-50 font-semibold text-3xl tracking-tight">POLEZ</h1>
      <p className="text-zinc-600 text-sm mt-1 mb-8">Audio Forensics & Sanitization</p>

      {loading ? (
        <div className="w-64 space-y-2">
          <div className="text-zinc-400 text-sm text-center font-data">
            Uploading... {progress}%
          </div>
          <div className="w-full bg-zinc-800 rounded-full h-1.5" role="progressbar" aria-valuenow={progress} aria-valuemin={0} aria-valuemax={100} aria-label="Upload progress">
            <div
              className="bg-zinc-400 h-1.5 rounded-full transition-all duration-200"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      ) : (
        <div
          className={`flex flex-col items-center gap-4 border border-dashed rounded-[6px] px-12 py-8 transition-colors cursor-pointer ${
            dragOver
              ? 'border-solid border-zinc-700 bg-zinc-900/30'
              : 'border-zinc-800 hover:border-zinc-700'
          }`}
          onClick={() => inputRef.current?.click()}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') inputRef.current?.click(); }}
          tabIndex={0}
          role="button"
          aria-label="Drop audio file or click to browse"
        >
          <p className="text-zinc-500 text-sm">Drop audio file or click to browse</p>
          <div className="flex gap-2">
            {FORMATS.map((fmt) => (
              <span
                key={fmt}
                className="font-data text-zinc-600 text-[0.65rem] bg-zinc-900 border border-zinc-800 rounded px-1.5 py-0.5"
              >
                {fmt}
              </span>
            ))}
          </div>
          <p className="text-zinc-700 text-xs">Max {formatBytes(MAX_UPLOAD_BYTES)}</p>
        </div>
      )}

      <input
        ref={inputRef}
        type="file"
        accept="audio/*"
        onChange={handlePickerChange}
        className="hidden"
        aria-label="Choose audio file"
      />

      {error && <span className="text-red-400 text-sm mt-4" role="alert">{error}</span>}
    </div>
  );
}
