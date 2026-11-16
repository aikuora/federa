// Creating an organization in a new folder, or opening one, and opening again on start the one the
// person had open last, since a runtime that restarted has none open.

import { open } from '@tauri-apps/plugin-dialog';
import type { Command } from 'protocol/Command';
import type { Organization } from 'protocol/Organization';
import { type FormEvent, useState } from 'react';

import { describe, request } from '../runtime/index.ts';
import { Button } from '../ui/button.tsx';
import { Input } from '../ui/input.tsx';

const LAST_FOLDER = 'organization-folder';

/** The organization the person had open last, opened again; `null` when there is none to open. */
export async function reopen(): Promise<Organization | null> {
  const folder = localStorage.getItem(LAST_FOLDER);
  if (folder === null) return null;
  try {
    return await openWith({ command: 'open_organization', folder });
  } catch {
    // The folder may be gone or no longer hold an organization; the person picks one again.
    return null;
  }
}

async function openWith(command: Command): Promise<Organization | null> {
  const reply = await request(command);
  if (reply.reply !== 'organization') return null;
  localStorage.setItem(LAST_FOLDER, reply.organization.folder);
  return reply.organization;
}

export function OrganizationSetup({ onOpen }: { onOpen: (organization: Organization) => void }) {
  const [name, setName] = useState('');
  const [error, setError] = useState<string>();

  async function run(title: string, command: (folder: string) => Command) {
    const folder = await open({ directory: true, title });
    if (folder === null) return;
    setError(undefined);
    try {
      const organization = await openWith(command(folder));
      if (organization) onOpen(organization);
    } catch (failure) {
      setError(describe(failure));
    }
  }

  function create(event: FormEvent) {
    event.preventDefault();
    void run('Elige una carpeta nueva y vacía para la organización', (folder) => ({
      command: 'create_organization',
      folder,
      name: name.trim(),
    }));
  }

  return (
    <main className="mx-auto flex max-w-md flex-col gap-8 px-6 py-16">
      <h1 className="text-2xl font-semibold">Federa</h1>
      <form onSubmit={create} className="flex flex-col gap-3">
        <h2 className="font-medium">Crear una organización</h2>
        <Input
          placeholder="Nombre de la organización"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <Button type="submit" disabled={name.trim() === ''}>
          Elegir carpeta y crear
        </Button>
      </form>
      <section className="flex flex-col gap-3">
        <h2 className="font-medium">Abrir una organización</h2>
        <Button
          variant="outline"
          onClick={() =>
            void run('Elige la carpeta de la organización', (folder) => ({
              command: 'open_organization',
              folder,
            }))
          }
        >
          Elegir carpeta
        </Button>
      </section>
      {error && <p className="text-sm text-destructive">{error}</p>}
    </main>
  );
}
