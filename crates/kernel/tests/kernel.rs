use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as Process,
};

use federation::Federation;
use kernel::{AgentIdentity, Kernel, Role, ToolError};
use protocol::{
    Asset, AssetName, Command, Failure, FailureCode, Organization, Outcome, Reply, Stage,
};
use serde_json::json;

async fn done(kernel: &Kernel, command: Command) -> Reply {
    match kernel.handle(command).await {
        Outcome::Done { reply } => reply,
        Outcome::Failed { failure } => panic!("{failure:?}"),
    }
}

async fn failed(kernel: &Kernel, command: Command) -> Failure {
    match kernel.handle(command).await {
        Outcome::Failed { failure } => failure,
        Outcome::Done { reply } => panic!("{reply:?}"),
    }
}

/// A git repository holding the asset `billing` in `services/billing`, as in a monorepo.
fn platform(dir: &Path) -> PathBuf {
    let repo = dir.join("platform");
    let federa = repo.join("services/billing/.federa");
    fs::create_dir_all(&federa).unwrap();
    fs::write(
        federa.join("asset.toml"),
        "schema = 1\nname = \"billing\"\n",
    )
    .unwrap();
    fs::write(federa.join("knowledge.md"), "# billing\n").unwrap();
    // A git hook sets these for the repository it runs in, and git would init that one instead.
    let init = Process::new("git")
        .arg("init")
        .arg(&repo)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(init.status.success());
    repo
}

#[tokio::test]
async fn creates_an_organization_and_registers_an_asset() {
    let dir = tempfile::tempdir().unwrap();
    let repo = platform(dir.path());
    let billing = repo.join("services/billing");
    let acme = dir.path().join("acme");
    let kernel = Kernel::new("ana@acme.dev".to_owned());

    let none = failed(&kernel, Command::ListAssets).await;
    assert_eq!(none.code, FailureCode::NoOrganization);
    let conversation = Command::LoadConversation {
        asset: AssetName("billing".to_owned()),
    };
    assert_eq!(
        failed(&kernel, conversation).await.code,
        FailureCode::Internal
    );

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

    let kernel = Kernel::new("ana@acme.dev".to_owned());
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
    let kernel = Kernel::new("ana@acme.dev".to_owned());
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
    let kernel = Kernel::new("ana@acme.dev".to_owned());
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

fn peer_on(asset: &str) -> AgentIdentity {
    AgentIdentity {
        role: Role::Peer,
        session: "peer-1".to_owned(),
        asset: federation::AssetName::parse(asset).unwrap(),
        goal: None,
    }
}

#[tokio::test]
async fn an_agent_reads_the_knowledge_of_its_own_asset() {
    let dir = tempfile::tempdir().unwrap();
    let repo = platform(dir.path());
    let kernel = Kernel::new("ana@acme.dev".to_owned());
    let early = kernel.admit(peer_on("billing"));
    assert_eq!(
        kernel
            .call(early.as_str(), "asset_knowledge", json!({}))
            .await,
        Err(ToolError::Failed("No organization is open.".to_owned()))
    );

    done(
        &kernel,
        Command::CreateOrganization {
            folder: dir.path().join("acme"),
            name: "Acme".to_owned(),
        },
    )
    .await;
    done(
        &kernel,
        Command::RegisterAsset {
            folder: repo.join("services/billing"),
        },
    )
    .await;
    let token = kernel.admit(peer_on("billing"));
    let tools = kernel.tools(token.as_str()).unwrap();
    assert_eq!(
        tools.iter().map(|tool| tool.name).collect::<Vec<_>>(),
        ["asset_knowledge"]
    );
    assert_eq!(
        kernel
            .call(token.as_str(), "asset_knowledge", json!({}))
            .await,
        Ok("# billing\n".to_owned())
    );
    assert!(matches!(
        kernel
            .call(token.as_str(), "asset_knowledge", json!([]))
            .await,
        Err(ToolError::Invalid(_))
    ));
    assert_eq!(
        kernel.call(token.as_str(), "deploy", json!({})).await,
        Err(ToolError::UnknownTool("deploy".to_owned()))
    );

    let unregistered = kernel.admit(peer_on("ledger"));
    let failed = kernel.call(unregistered.as_str(), "asset_knowledge", json!({}));
    assert!(matches!(failed.await, Err(ToolError::Failed(message)) if message.contains("ledger")));
}

#[tokio::test]
async fn refuses_a_token_it_did_not_admit() {
    let kernel = Kernel::new("ana@acme.dev".to_owned());
    let token = kernel.admit(peer_on("billing"));
    let guess = "0".repeat(token.as_str().len());
    assert_eq!(kernel.tools(&guess), Err(ToolError::Unauthorized));
    assert_eq!(
        kernel.call(&guess, "asset_knowledge", json!({})).await,
        Err(ToolError::Unauthorized)
    );
    assert_ne!(kernel.admit(peer_on("billing")), token);
}

#[tokio::test]
async fn opening_an_organization_dismisses_every_agent() {
    let dir = tempfile::tempdir().unwrap();
    let acme = dir.path().join("acme");
    let kernel = Kernel::new("ana@acme.dev".to_owned());
    let create = Command::CreateOrganization {
        folder: acme.clone(),
        name: "Acme".to_owned(),
    };
    done(&kernel, create).await;
    let token = kernel.admit(peer_on("billing"));
    assert!(kernel.tools(token.as_str()).is_ok());

    done(&kernel, Command::OpenOrganization { folder: acme }).await;
    assert_eq!(kernel.tools(token.as_str()), Err(ToolError::Unauthorized));
    // A failed open leaves the organization open, and its agents with it.
    let token = kernel.admit(peer_on("billing"));
    let not_one = Command::OpenOrganization {
        folder: dir.path().to_owned(),
    };
    failed(&kernel, not_one).await;
    assert!(kernel.tools(token.as_str()).is_ok());
}
