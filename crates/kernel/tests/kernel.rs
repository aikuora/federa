mod common;

use std::{
    fs,
    path::{Path, PathBuf},
};

use common::{done, failed, platform};
use federation::Federation;
use kernel::Kernel;
use protocol::{Asset, AssetName, Command, FailureCode, Organization, Reply, Stage};

#[tokio::test]
async fn creates_an_organization_and_registers_an_asset() {
    let dir = tempfile::tempdir().unwrap();
    let repo = platform(dir.path());
    let billing = repo.join("services/billing");
    let acme = dir.path().join("acme");
    let kernel = Kernel::new(
        "ana@acme.dev".to_owned(),
        Err("no engine in this test".to_owned()),
        "conversations".into(),
        "http://127.0.0.1/mcp".to_owned(),
    );

    let none = failed(&kernel, Command::ListAssets).await;
    assert_eq!(none.code, FailureCode::NoOrganization);

    let reply = done(
        &kernel,
        Command::CreateOrganization {
            folder: acme.clone(),
            name: " Acme ".to_owned(),
        },
    )
    .await;
    let organization = Organization {
        name: "Acme".to_owned(),
        folder: acme.clone(),
    };
    assert_eq!(
        reply,
        Reply::Organization {
            organization: organization.clone()
        }
    );

    let asset = Asset {
        name: AssetName("billing".to_owned()),
        stage: Stage::Development,
        managed: true,
    };
    let register = || Command::RegisterAsset {
        folder: billing.clone(),
    };
    assert_eq!(
        done(&kernel, register()).await,
        Reply::Asset {
            asset: asset.clone()
        }
    );
    assert_eq!(
        failed(&kernel, register()).await.code,
        FailureCode::AlreadyExists
    );

    let kernel = Kernel::new(
        "ana@acme.dev".to_owned(),
        Err("no engine in this test".to_owned()),
        "conversations".into(),
        "http://127.0.0.1/mcp".to_owned(),
    );
    assert_eq!(
        done(
            &kernel,
            Command::OpenOrganization {
                folder: acme.clone()
            }
        )
        .await,
        Reply::Organization { organization }
    );
    assert_eq!(
        done(&kernel, Command::ListAssets).await,
        Reply::Assets {
            assets: vec![asset]
        }
    );

    let entry = &Federation::open(&acme).unwrap().assets().unwrap()[0];
    assert_eq!(entry.owner, "ana@acme.dev");
    assert_eq!(
        Path::new(&entry.location.repo),
        fs::canonicalize(&repo).unwrap()
    );
    assert_eq!(entry.location.folder, PathBuf::from("services/billing"));
}

#[tokio::test]
async fn refuses_an_organization_without_a_name() {
    let dir = tempfile::tempdir().unwrap();
    let kernel = Kernel::new(
        "ana@acme.dev".to_owned(),
        Err("no engine in this test".to_owned()),
        "conversations".into(),
        "http://127.0.0.1/mcp".to_owned(),
    );
    let failure = failed(
        &kernel,
        Command::CreateOrganization {
            folder: dir.path().join("acme"),
            name: " \t".to_owned(),
        },
    )
    .await;
    assert_eq!(failure.code, FailureCode::Invalid);
    assert!(!dir.path().join("acme").exists());
}

#[tokio::test]
async fn refuses_an_asset_outside_a_git_repository() {
    let dir = tempfile::tempdir().unwrap();
    let billing = dir.path().join("billing/.federa");
    fs::create_dir_all(&billing).unwrap();
    fs::write(
        billing.join("asset.toml"),
        "schema = 1\nname = \"billing\"\n",
    )
    .unwrap();
    fs::write(billing.join("knowledge.md"), "").unwrap();
    let kernel = Kernel::new(
        "ana@acme.dev".to_owned(),
        Err("no engine in this test".to_owned()),
        "conversations".into(),
        "http://127.0.0.1/mcp".to_owned(),
    );
    done(
        &kernel,
        Command::CreateOrganization {
            folder: dir.path().join("acme"),
            name: "Acme".to_owned(),
        },
    )
    .await;
    let failure = failed(
        &kernel,
        Command::RegisterAsset {
            folder: dir.path().join("billing"),
        },
    )
    .await;
    assert_eq!(failure.code, FailureCode::Invalid);
    assert!(failure.message.contains("billing"), "{}", failure.message);
}
