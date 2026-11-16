use std::fs;

use federation::{AssetName, CatalogEntry, Error, Federation, Location, Stage, read_asset};

#[test]
fn registers_an_asset_and_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("acme-federation");
    let asset = dir.path().join("billing");
    fs::create_dir_all(asset.join(".federa")).unwrap();
    fs::write(
        asset.join(".federa/asset.toml"),
        "schema = 1\nname = \"billing\"\n",
    )
    .unwrap();
    fs::write(
        asset.join(".federa/knowledge.md"),
        "# billing\n\nCharges customers.\n",
    )
    .unwrap();

    let federation = Federation::create(&root, "Acme").unwrap();
    assert!(root.join(".git").is_dir());
    let folder = read_asset(&asset).unwrap();
    let entry = CatalogEntry {
        name: folder.name.clone(),
        owner: "ana".to_owned(),
        location: Location {
            repo: asset.to_string_lossy().into_owned(),
            folder: "".into(),
        },
        stage: Stage::Development,
        managed: true,
    };
    federation.register(&entry).unwrap();
    assert!(matches!(
        federation.register(&entry),
        Err(Error::AlreadyRegistered(_))
    ));

    let federation = Federation::open(&root).unwrap();
    assert_eq!(federation.name(), "Acme");
    assert_eq!(federation.knowledge().unwrap(), "");
    assert_eq!(federation.assets().unwrap(), std::slice::from_ref(&entry));
    let name = AssetName::parse("billing").unwrap();
    assert_eq!(federation.asset(&name).unwrap(), entry);
    let folder = read_asset(&asset).unwrap();
    assert_eq!(folder.name, name);
    assert_eq!(folder.knowledge, "# billing\n\nCharges customers.\n");
}
