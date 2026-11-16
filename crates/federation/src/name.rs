use std::fmt;

use crate::Error;

/// An asset's name: lowercase kebab-case, because it names the asset's file in the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetName(String);

impl AssetName {
    pub fn parse(name: &str) -> Result<Self, Error> {
        let valid = name.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
        if !valid {
            return Err(Error::InvalidName(name.to_owned()));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_lowercase_kebab_case() {
        for name in ["api", "design-system", "v2", "auth-2fa-service"] {
            assert_eq!(AssetName::parse(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn rejects_anything_else() {
        for name in [
            "",
            "-api",
            "api-",
            "design--system",
            "Api",
            "design_system",
            "design system",
            "api.v2",
            "../api",
            "ápi",
        ] {
            assert!(
                matches!(AssetName::parse(name), Err(Error::InvalidName(_))),
                "{name:?}"
            );
        }
    }
}
