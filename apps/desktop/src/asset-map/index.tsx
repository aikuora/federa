// The open organization's assets, and registering a new one from the folder that holds its
// `.federa/`.

import { open } from '@tauri-apps/plugin-dialog';
import type { Asset } from 'protocol/Asset';
import type { AssetName } from 'protocol/AssetName';
import { useEffect, useState } from 'react';

import { describe, request } from '../runtime/index.ts';
import { Button } from '../ui/button.tsx';
import { cn } from '../ui/cn.ts';

const STAGES = { development: 'en desarrollo', production: 'productivo' } as const;

export function AssetMap({
  selected,
  onSelect,
}: {
  selected: AssetName | undefined;
  onSelect: (asset: AssetName) => void;
}) {
  const [assets, setAssets] = useState<Asset[]>([]);
  const [error, setError] = useState<string>();

  useEffect(() => {
    request({ command: 'list_assets' }).then(
      (reply) => reply.reply === 'assets' && setAssets(reply.assets),
      (failure) => setError(describe(failure)),
    );
  }, []);

  async function register() {
    const folder = await open({ directory: true, title: 'Elige la carpeta del activo' });
    if (folder === null) return;
    setError(undefined);
    try {
      const reply = await request({ command: 'register_asset', folder });
      if (reply.reply !== 'asset') return;
      setAssets((current) => [...current, reply.asset]);
      onSelect(reply.asset.name);
    } catch (failure) {
      setError(describe(failure));
    }
  }

  return (
    <section className="flex flex-col gap-2">
      <h2 className="px-2 text-xs font-medium text-muted-foreground uppercase">Activos</h2>
      <ul className="flex flex-col gap-1">
        {assets.map((asset) => (
          <li key={asset.name}>
            <button
              onClick={() => onSelect(asset.name)}
              className={cn(
                'flex w-full flex-col items-start rounded-md px-2 py-1.5 text-left hover:bg-accent',
                asset.name === selected && 'bg-accent',
              )}
            >
              <span className="text-sm font-medium">{asset.name}</span>
              <span className="text-xs text-muted-foreground">
                {asset.managed ? STAGES[asset.stage] : 'externo'}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <Button variant="outline" size="sm" onClick={() => void register()}>
        Registrar activo
      </Button>
      {error && <p className="px-2 text-sm text-destructive">{error}</p>}
    </section>
  );
}
