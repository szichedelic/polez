import { useState, useRef, useCallback } from 'react';
import { batchClean, getBatchDownloadUrl, MAX_UPLOAD_BYTES } from '../api/client';
import type { BatchFileResult } from '../api/client';

interface BatchFile {
  file: File;
  status: 'pending' | 'processing' | 'done' | 'error';
  result?: BatchFileResult;
  downloadId?: string;
}

export function BatchPanel() {
  const [files, setFiles] = useState<BatchFile[]>([]);
  const [mode, setMode] = useState('standard');
  const [processing, setProcessing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const addFiles = useCallback((newFiles: FileList | File[]) => {
    const valid: BatchFile[] = [];
    for (const file of Array.from(newFiles)) {
      if (file.size > MAX_UPLOAD_BYTES) continue;
      valid.push({ file, status: 'pending' });
    }
    setFiles(prev => [...prev, ...valid]);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (e.dataTransfer.files.length > 0) {
      addFiles(e.dataTransfer.files);
    }
  }, [addFiles]);

  const handleProcess = async () => {
    if (files.length === 0) return;
    setProcessing(true);
    setError(null);

    setFiles(prev => prev.map(f => ({ ...f, status: 'processing' as const })));

    try {
      const rawFiles = files.map(f => f.file);
      const response = await batchClean(rawFiles, mode);

      setFiles(prev => prev.map((f, i) => {
        const result = response.results[i];
        if (!result) return f;
        return {
          ...f,
          status: result.success ? 'done' as const : 'error' as const,
          result,
          downloadId: result.download_id ?? undefined,
        };
      }));
    } catch (e: any) {
      setError(e.message || 'Batch processing failed');
      setFiles(prev => prev.map(f => ({ ...f, status: 'error' as const })));
    } finally {
      setProcessing(false);
    }
  };

  const removeFile = (index: number) => {
    setFiles(prev => prev.filter((_, i) => i !== index));
  };

  const clearAll = () => {
    setFiles([]);
    setError(null);
  };

  const statusIcon = (status: BatchFile['status']) => {
    switch (status) {
      case 'pending': return '\u25CB';
      case 'processing': return '\u25CE';
      case 'done': return '\u2713';
      case 'error': return '\u2717';
    }
  };

  const statusColor = (status: BatchFile['status']) => {
    switch (status) {
      case 'pending': return 'text-zinc-500';
      case 'processing': return 'text-yellow-400';
      case 'done': return 'text-green-400';
      case 'error': return 'text-red-400';
    }
  };

  return (
    <section className="bg-zinc-900 border border-zinc-700 rounded p-4" aria-label="Batch processing">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">BATCH PROCESSING</span>
        <div className="flex flex-wrap gap-2 items-center">
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value)}
            disabled={processing}
            className="bg-zinc-800 text-zinc-200 border border-zinc-600 rounded px-2 py-2 sm:py-1 text-xs min-h-[44px] sm:min-h-0"
            aria-label="Select batch cleaning mode"
          >
            <option value="fast">Fast</option>
            <option value="standard">Standard</option>
            <option value="preserving">Preserving</option>
            <option value="aggressive">Aggressive</option>
          </select>
          <button
            onClick={handleProcess}
            disabled={processing || files.length === 0}
            className="bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white px-3 py-2 sm:py-1 rounded text-xs font-medium min-h-[44px] sm:min-h-0"
            aria-label={`Clean ${files.length} file${files.length !== 1 ? 's' : ''}`}
          >
            {processing ? 'Processing...' : `Clean ${files.length} file${files.length !== 1 ? 's' : ''}`}
          </button>
          {files.length > 0 && !processing && (
            <button
              onClick={clearAll}
              className="text-zinc-500 hover:text-zinc-300 text-xs min-h-[44px] sm:min-h-0"
            >
              Clear
            </button>
          )}
        </div>
      </div>

      {error && <p className="text-red-400 text-sm mb-2" role="alert">{error}</p>}

      <div
        onDrop={handleDrop}
        onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        className={`border border-dashed rounded p-4 mb-3 text-center transition-colors ${
          dragOver ? 'border-zinc-500 bg-zinc-800/50' : 'border-zinc-700 hover:border-zinc-600'
        }`}
      >
        <div className="text-zinc-400 text-sm mb-1">
          Drag & drop multiple audio files here
        </div>
        <button
          onClick={() => inputRef.current?.click()}
          disabled={processing}
          className="text-zinc-400 hover:text-zinc-300 text-xs min-h-[44px] sm:min-h-0"
        >
          or choose files
        </button>
        <input
          ref={inputRef}
          type="file"
          accept="audio/*"
          multiple
          onChange={(e) => { if (e.target.files) addFiles(e.target.files); }}
          className="hidden"
          aria-label="Choose audio files for batch processing"
        />
      </div>

      {files.length > 0 && (
        <div className="space-y-1">
          {files.map((f, i) => (
            <div key={i} className="flex items-center gap-2 text-xs bg-zinc-800 rounded px-2 py-1.5">
              <span className={statusColor(f.status)}>{statusIcon(f.status)}</span>
              <span className="text-zinc-200 flex-1 truncate">{f.file.name}</span>
              <span className="text-zinc-500 font-data">{(f.file.size / 1024 / 1024).toFixed(1)} MB</span>
              {f.result?.processing_time != null && (
                <span className="text-zinc-500 font-data">{f.result.processing_time.toFixed(1)}s</span>
              )}
              {f.result?.quality_loss != null && (
                <span className="text-zinc-500 font-data">QL: {f.result.quality_loss.toFixed(2)}%</span>
              )}
              {f.result?.error && (
                <span className="text-red-400 truncate max-w-[200px]">{f.result.error}</span>
              )}
              {f.downloadId && (
                <a
                  href={getBatchDownloadUrl(f.downloadId)}
                  className="text-blue-400 hover:text-blue-300"
                  download
                >
                  Download
                </a>
              )}
              {f.status === 'pending' && !processing && (
                <button
                  onClick={() => removeFile(i)}
                  className="text-zinc-500 hover:text-zinc-300"
                  aria-label={`Remove ${f.file.name}`}
                >
                  {'\u2717'}
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {files.length === 0 && (
        <p className="text-zinc-500 text-sm text-center">No files queued</p>
      )}
    </section>
  );
}
