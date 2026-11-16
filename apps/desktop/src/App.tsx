import type { AssetName } from 'protocol/AssetName';
import type { Organization } from 'protocol/Organization';
import { useState } from 'react';

import { AssetConversation } from './asset-conversation/index.tsx';
import { AssetMap } from './asset-map/index.tsx';
import { ClaudeConnection } from './claude-connection/index.tsx';
import { OrganizationSetup } from './organization-setup/index.tsx';

export function App({ reopened }: { reopened: Organization | null }) {
  const [organization, setOrganization] = useState(reopened);
  const [asset, setAsset] = useState<AssetName>();

  if (organization === null) return <OrganizationSetup onOpen={setOrganization} />;
  return (
    <div className="flex h-screen">
      <aside className="flex w-64 shrink-0 flex-col gap-4 border-r p-3">
        <p className="px-2 font-semibold">{organization.name}</p>
        <AssetMap selected={asset} onSelect={setAsset} />
        <div className="mt-auto">
          <ClaudeConnection />
        </div>
      </aside>
      <main className="flex-1">
        {asset === undefined ? (
          <p className="p-6 text-sm text-muted-foreground">
            Elige un activo para conversar con tu peer sobre él.
          </p>
        ) : (
          <AssetConversation asset={asset} />
        )}
      </main>
    </div>
  );
}
