# federation

How an organization's federation repository and each asset's `.federa/` folder are stored: their
layout and file formats, hidden behind one interface so the rest of the core never touches a path
or a file inside them.

## Scope

### Responsibilities

- **federation-repository**: Creating and opening an organization's federation repository, with the
  organization's knowledge.
- **catalog**: The catalog of assets: each asset's owner, where its code lives, its stage and whether
  it is managed.
- **asset-folder**: Reading what a managed asset says about itself in its `.federa/` folder.

### Out of scope

- Deciding anything about an organization or an asset, which `kernel` does.

### Design guidelines

- Structured files are TOML and carry their schema number; knowledge is Markdown.
- A file format changes in place until the first published version of the app.

## Public interface

- `Federation`: creates or opens a federation repository, reads the organization's knowledge and
  reads and writes the catalog.
- `CatalogEntry`, `Location` and `Stage`: an asset as the catalog records it.
- `Location::find`: where the code in a folder lives, as the root of the git repository that holds
  it and the folder relative to that root.
- `read_asset` and `AssetFolder`: what an asset's `.federa/` says.
- `AssetName`: a valid asset name.
- `Error`: why an operation failed.

## Development

### Testing

- The tests create federation repositories in temporary folders and need the `git` binary.
