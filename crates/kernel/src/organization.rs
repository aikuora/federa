use std::{fs, path::Path};

use federation::Federation;
use protocol::{Failure, FailureCode, Organization};

use crate::failure;

pub(crate) fn create(folder: &Path, name: &str) -> Result<Federation, Failure> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Failure {
            code: FailureCode::Invalid,
            message: "An organization needs a name.".to_owned(),
        });
    }
    Federation::create(folder, name).map_err(failure::from_federation)
}

pub(crate) fn open(folder: &Path) -> Result<Federation, Failure> {
    Federation::open(folder).map_err(failure::from_federation)
}

/// Whether both are the same organization, its folder spelled either way.
pub(crate) fn same(one: &Federation, other: &Federation) -> bool {
    match (fs::canonicalize(one.root()), fs::canonicalize(other.root())) {
        (Ok(one), Ok(other)) => one == other,
        _ => false,
    }
}

/// The organization as a client sees it.
pub(crate) fn summary(federation: &Federation) -> Organization {
    Organization {
        name: federation.name().to_owned(),
        folder: federation.root().to_owned(),
    }
}
