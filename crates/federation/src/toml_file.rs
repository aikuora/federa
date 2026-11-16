use std::{fs, io::Write, path::Path};

use serde::{Serialize, de::DeserializeOwned};

use crate::Error;

// Every TOML file starts with `schema = 1`. The number is checked before the rest of the file, so a
// file in another schema reads as that and not as a missing field.
const SCHEMA: i64 = 1;

pub(crate) fn read<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let text = fs::read_to_string(path).map_err(Error::io(path))?;
    parse(&text).map_err(|reason| Error::invalid(path, reason))
}

/// Writes a new file, and fails with `AlreadyExists` if there is one. A file left half written is
/// removed, so writing it again finds no file in the way.
pub(crate) fn create<T: Serialize>(path: &Path, body: &T) -> Result<(), Error> {
    let body = toml::to_string(body).map_err(|e| Error::invalid(path, e))?;
    let mut file = fs::File::create_new(path).map_err(Error::io(path))?;
    file.write_all(format!("schema = {SCHEMA}\n{body}").as_bytes())
        .map_err(|e| {
            let _ = fs::remove_file(path);
            Error::io(path)(e)
        })
}

fn parse<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let mut table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    match table.remove("schema") {
        Some(toml::Value::Integer(SCHEMA)) => {}
        Some(other) => return Err(format!("schema {other} is not one this version reads")),
        None => return Err("it has no schema number".to_owned()),
    }
    table.try_into().map_err(|e: toml::de::Error| e.to_string())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Body {
        name: String,
    }

    #[test]
    fn writes_the_schema_and_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body.toml");
        let body = Body {
            name: "acme".to_owned(),
        };
        create(&path, &body).unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .starts_with("schema = 1\n")
        );
        assert_eq!(read::<Body>(&path).unwrap(), body);
    }

    #[test]
    fn create_refuses_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("body.toml");
        fs::write(&path, "").unwrap();
        let body = Body {
            name: "acme".to_owned(),
        };
        assert!(
            matches!(create(&path, &body), Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::AlreadyExists)
        );
    }

    #[test]
    fn rejects_a_file_without_schema_one() {
        for text in [
            "name = \"acme\"",
            "schema = 2\nname = \"acme\"",
            "schema = \"1\"\nname = \"acme\"",
        ] {
            assert!(
                parse::<Body>(text).unwrap_err().contains("schema"),
                "{text:?}"
            );
        }
    }

    #[test]
    fn rejects_text_that_is_not_toml() {
        assert!(parse::<Body>("schema = 1\nname = ").is_err());
    }

    #[test]
    fn rejects_a_missing_field() {
        assert!(parse::<Body>("schema = 1").unwrap_err().contains("name"));
    }
}
