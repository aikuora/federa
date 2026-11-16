use protocol::{Failure, FailureCode};

/// What a client is told when the federation repository refuses or fails an operation.
pub(crate) fn from_federation(error: federation::Error) -> Failure {
    use federation::Error::*;
    // No wildcard: a new error has to choose the code a client acts on. `Invalid` covers anything
    // the person can fix in a folder or a file the message names; `Internal`, what they cannot.
    let code = match &error {
        NotEmpty(_)
        | NotFederation(_)
        | NoAssetFolder(_)
        | NotInRepository { .. }
        | InvalidName(_)
        | Invalid { .. } => FailureCode::Invalid,
        AlreadyRegistered(_) => FailureCode::AlreadyExists,
        NotRegistered(_) => FailureCode::NotFound,
        Git(_) | Io { .. } => FailureCode::Internal,
    };
    Failure {
        code,
        message: error.to_string(),
    }
}

/// What a client is told when a peer's turn fails in its engine.
pub(crate) fn from_engine(failure: engine::Failure) -> Failure {
    use engine::Failure::*;
    let code = match &failure {
        NotSignedIn { .. } => FailureCode::NotSignedIn,
        Unreadable(_) | Engine(_) => FailureCode::Internal,
    };
    Failure {
        code,
        message: failure.to_string(),
    }
}

/// What a client is told when the engine cannot open a peer or take a message.
pub(crate) fn from_engine_error(error: engine::Error) -> Failure {
    Failure {
        code: FailureCode::Internal,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use federation::{AssetName, Error};

    use super::*;

    #[test]
    fn every_error_gets_the_code_a_client_acts_on() {
        let billing = || AssetName::parse("billing").unwrap();
        for (error, code) in [
            (Error::NotEmpty("/work/acme".into()), FailureCode::Invalid),
            (
                Error::NotFederation("/work/acme".into()),
                FailureCode::Invalid,
            ),
            (
                Error::NoAssetFolder("/src/billing".into()),
                FailureCode::Invalid,
            ),
            (
                Error::NotInRepository {
                    folder: "/src/billing".into(),
                    reason: "fatal: not a git repository".into(),
                },
                FailureCode::Invalid,
            ),
            (Error::InvalidName("Billing".into()), FailureCode::Invalid),
            (
                Error::Invalid {
                    file: "/src/billing/.federa/asset.toml".into(),
                    reason: "schema 2 is not one this version reads".into(),
                },
                FailureCode::Invalid,
            ),
            (
                Error::AlreadyRegistered(billing()),
                FailureCode::AlreadyExists,
            ),
            (Error::NotRegistered(billing()), FailureCode::NotFound),
            (
                Error::Git("could not run git".into()),
                FailureCode::Internal,
            ),
            (
                Error::Io {
                    path: "/work/acme/assets".into(),
                    source: io::Error::from(io::ErrorKind::PermissionDenied),
                },
                FailureCode::Internal,
            ),
        ] {
            let message = error.to_string();
            assert_eq!(from_federation(error), Failure { code, message });
        }
    }
}
