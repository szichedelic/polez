import { useEffect, useState } from 'react';
import { getBitPlane, type BitPlaneData } from '../api/client';
import { useColorblind } from '../hooks/useColorblind';
import { Card } from './Card';

interface Props {
  fileLoaded: boolean;
}

export function BitPlaneViewer({ fileLoaded }: Props) {
  const { palette } = useColorblind();
  const [data, setData] = useState<BitPlaneData | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!fileLoaded) return;
    setLoading(true);
    getBitPlane().then(setData).finally(() => setLoading(false));
  }, [fileLoaded]);

  return (
    <Card label="Bit plane analysis">
      <div className="flex items-center justify-between mb-3">
        <span className="font-heading text-zinc-600 text-[0.65rem] font-medium uppercase tracking-[0.18em]">BIT PLANES</span>
        {loading && <span className="text-zinc-400 text-xs" aria-live="polite">Loading...</span>}
      </div>

      {!data && !loading && (
        <p className="text-zinc-500 text-sm">Load a file to view bit planes</p>
      )}

      {data && (
        <div className="space-y-2">
          {data.planes.map((plane) => {
            const biased = plane.bias > 0.02;
            const barWidth = plane.ones_ratio * 100;

            return (
              <div key={plane.bit} className="flex items-center gap-2">
                <span className="text-zinc-500 text-xs w-20">
                  Plane {plane.bit} {plane.bit === 0 ? '(LSB)' : plane.bit === 7 ? '(MSB)' : ''}
                </span>
                <div className="flex-1 h-3 bg-zinc-700 rounded-full overflow-hidden" role="progressbar" aria-valuenow={barWidth} aria-valuemin={0} aria-valuemax={100} aria-label={`Plane ${plane.bit} ones ratio: ${(plane.ones_ratio * 100).toFixed(2)}%${biased ? ' (biased)' : ''}`}>
                  <div
                    className={`h-full rounded-full ${biased ? palette.biased.bg : palette.normal.bg}`}
                    style={{ width: `${barWidth}%` }}
                  />
                </div>
                <span className={`text-xs w-20 text-right font-data ${biased ? palette.biased.text : palette.normal.text}`}>
                  {biased ? '\u26A0 ' : ''}{(plane.ones_ratio * 100).toFixed(2)}%
                </span>
              </div>
            );
          })}
          <div className="text-xs text-zinc-500 mt-2">
            {data.planes.filter(p => p.bias > 0.02).length}/8 planes show bias (suspicious if many)
          </div>
        </div>
      )}
    </Card>
  );
}
