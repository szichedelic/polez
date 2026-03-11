import { useState } from 'react';
import { analyzeMetadata } from '../api/client';
import { Card } from './Card';

interface TagInfo {
  tag_type: string;
  key: string;
  value: string;
  suspicious: boolean;
}

interface ChunkInfo {
  description: string;
  offset: number;
}

interface MetadataResult {
  tags: TagInfo[];
  suspicious_chunks: ChunkInfo[];
  anomalies: string[];
}

interface Props {
  fileLoaded: boolean;
  hasCleaned: boolean;
}

export function MetadataViewer({ fileLoaded, hasCleaned }: Props) {
  const [before, setBefore] = useState<MetadataResult | null>(null);
  const [after, setAfter] = useState<MetadataResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [showRemoved, setShowRemoved] = useState(true);

  const scan = async () => {
    setLoading(true);
    try {
      const result = await analyzeMetadata();
      setBefore(result);
      setAfter(null);
    } finally {
      setLoading(false);
    }
  };

  const removedTagKeys = new Set<string>();
  if (before && after) {
    const afterKeys = new Set(after.tags.map((t: TagInfo) => `${t.tag_type}:${t.key}`));
    for (const tag of before.tags) {
      const id = `${tag.tag_type}:${tag.key}`;
      if (!afterKeys.has(id)) removedTagKeys.add(id);
    }
  }

  if (!fileLoaded) return null;

  return (
    <Card label="Metadata tags viewer">
      <div className="flex flex-wrap items-center justify-between gap-2 mb-3">
        <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">METADATA TAGS</span>
        <div className="flex flex-wrap items-center gap-2">
          {hasCleaned && before && (
            <label className="flex items-center gap-1.5 text-xs text-zinc-400 cursor-pointer">
              <input
                type="checkbox"
                checked={showRemoved}
                onChange={() => setShowRemoved(!showRemoved)}
                className="accent-emerald-500"
              />
              Show removed
            </label>
          )}
          <button
            onClick={scan}
            disabled={loading}
            className="bg-zinc-700 hover:bg-zinc-600 disabled:opacity-50 text-zinc-200 px-3 py-2 sm:py-1 rounded text-xs min-h-[44px] sm:min-h-0"
            aria-label="Scan file for metadata tags"
          >
            {loading ? 'Scanning...' : 'Scan Tags'}
          </button>
        </div>
      </div>

      {!before && (
        <p className="text-zinc-500 text-sm">Click "Scan Tags" to inspect metadata</p>
      )}

      {before && before.tags.length === 0 && before.suspicious_chunks.length === 0 && (
        <p className="text-zinc-500 text-sm">No metadata tags found</p>
      )}

      {before && before.tags.length > 0 && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-zinc-500 border-b border-zinc-800">
                <th className="text-left py-1 pr-3 font-medium">Type</th>
                <th className="text-left py-1 pr-3 font-medium">Key</th>
                <th className="text-left py-1 font-medium">Value</th>
                {hasCleaned && after && <th className="text-left py-1 pl-3 font-medium w-20">Status</th>}
              </tr>
            </thead>
            <tbody>
              {before.tags.map((tag, i) => {
                const tagId = `${tag.tag_type}:${tag.key}`;
                const wasRemoved = removedTagKeys.has(tagId);
                if (wasRemoved && !showRemoved) return null;

                return (
                  <tr
                    key={i}
                    className={`border-b border-zinc-800/50 ${
                      wasRemoved ? 'opacity-50 line-through' : ''
                    } ${tag.suspicious ? 'bg-red-950/30' : ''}`}
                  >
                    <td className="py-1 pr-3 text-zinc-400 whitespace-nowrap">{tag.tag_type}</td>
                    <td className={`py-1 pr-3 whitespace-nowrap ${tag.suspicious ? 'text-red-400' : 'text-zinc-300'}`}>
                      {tag.key}
                      {tag.suspicious && (
                        <span className="ml-1.5 text-red-500 text-[10px]" title="Potentially tracking-related">!</span>
                      )}
                    </td>
                    <td className="py-1 text-zinc-400 max-w-xs truncate" title={tag.value}>
                      {tag.value}
                    </td>
                    {hasCleaned && after && (
                      <td className="py-1 pl-3">
                        {wasRemoved ? (
                          <span className="text-emerald-400">Removed</span>
                        ) : (
                          <span className="text-zinc-600">Kept</span>
                        )}
                      </td>
                    )}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {before && before.suspicious_chunks.length > 0 && (
        <div className="mt-3">
          <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">SUSPICIOUS CHUNKS</span>
          <div className="mt-1 space-y-1">
            {before.suspicious_chunks.map((chunk, i) => (
              <div key={i} className="flex justify-between text-xs bg-red-950/20 rounded px-2 py-1">
                <span className="text-red-400">{chunk.description}</span>
                <span className="text-zinc-500 font-data">offset 0x{chunk.offset.toString(16)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {before && before.anomalies.length > 0 && (
        <div className="mt-3">
          <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">ANOMALIES</span>
          <div className="mt-1 space-y-1">
            {before.anomalies.map((a, i) => (
              <div key={i} className="text-xs text-yellow-400">{a}</div>
            ))}
          </div>
        </div>
      )}
    </Card>
  );
}
