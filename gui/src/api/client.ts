const BASE = '';

export interface FileInfo {
  file_path: string;
  format: string;
  duration_secs: number;
  sample_rate: number;
  channels: number;
}

export interface WaveformData {
  min: number[];
  max: number[];
  sample_rate: number;
  duration_secs: number;
  channels: number;
}

export interface SpectrogramData {
  magnitudes: number[][];
  freq_min: number;
  freq_max: number;
  time_start: number;
  time_end: number;
  num_freq_bins: number;
  num_time_frames: number;
}

export interface PlaneSummary {
  bit: number;
  ones_ratio: number;
  bias: number;
}

export interface BitPlaneData {
  planes: PlaneSummary[];
}

export async function uploadFile(file: File): Promise<FileInfo> {
  const form = new FormData();
  form.append('file', file);
  const res = await fetch(`${BASE}/api/upload`, {
    method: 'POST',
    body: form,
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function loadFile(path: string): Promise<FileInfo> {
  const res = await fetch(`${BASE}/api/load`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ path }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getWaveform(width = 1200, start?: number, end?: number): Promise<WaveformData> {
  const params = new URLSearchParams({ width: String(width) });
  if (start !== undefined) params.set('start', String(start));
  if (end !== undefined) params.set('end', String(end));
  const res = await fetch(`${BASE}/api/waveform?${params}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getSpectrogram(opts?: {
  fft_size?: number;
  freq_min?: number;
  freq_max?: number;
  start?: number;
  duration?: number;
}): Promise<SpectrogramData> {
  const params = new URLSearchParams();
  if (opts?.fft_size) params.set('fft_size', String(opts.fft_size));
  if (opts?.freq_min !== undefined) params.set('freq_min', String(opts.freq_min));
  if (opts?.freq_max !== undefined) params.set('freq_max', String(opts.freq_max));
  if (opts?.start !== undefined) params.set('start', String(opts.start));
  if (opts?.duration !== undefined) params.set('duration', String(opts.duration));
  const res = await fetch(`${BASE}/api/spectrogram?${params}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getBitPlane(): Promise<BitPlaneData> {
  const res = await fetch(`${BASE}/api/bitplane`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function analyzeAll(): Promise<any> {
  const res = await fetch(`${BASE}/api/analyze/all`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function analyzeWatermark(): Promise<any> {
  const res = await fetch(`${BASE}/api/analyze/watermark`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function analyzePolez(): Promise<any> {
  const res = await fetch(`${BASE}/api/analyze/polez`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function analyzeStatistical(): Promise<any> {
  const res = await fetch(`${BASE}/api/analyze/statistical`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function analyzeMetadata(): Promise<any> {
  const res = await fetch(`${BASE}/api/analyze/metadata`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function getAudioUrl(): string {
  return `${BASE}/api/audio`;
}

export interface CleanResponse {
  success: boolean;
  metadata_removed: number;
  patterns_found: number;
  patterns_suppressed: number;
  quality_loss: number;
  processing_time: number;
  before: any;
  after: any;
}

export async function cleanFile(mode?: string): Promise<CleanResponse> {
  const res = await fetch(`${BASE}/api/clean`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ mode: mode || 'standard' }),
  });
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export async function getCleanedWaveform(width = 1200, start?: number, end?: number): Promise<WaveformData> {
  const params = new URLSearchParams({ width: String(width) });
  if (start !== undefined) params.set('start', String(start));
  if (end !== undefined) params.set('end', String(end));
  const res = await fetch(`${BASE}/api/waveform/cleaned?${params}`);
  if (!res.ok) throw new Error(await res.text());
  return res.json();
}

export function getCleanedAudioUrl(): string {
  return `${BASE}/api/audio/cleaned`;
}

export async function saveCleanedFile(): Promise<void> {
  const res = await fetch(`${BASE}/api/save`, { method: 'POST' });
  if (!res.ok) throw new Error(await res.text());
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'cleaned_output.wav';
  a.click();
  URL.revokeObjectURL(url);
}
