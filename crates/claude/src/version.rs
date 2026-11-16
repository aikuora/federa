use std::fmt;

// The oldest version whose stream-json output the recordings capture. Newer ones only add fields.
pub(crate) const MINIMUM: Version = Version {
    major: 2,
    minor: 1,
    patch: 311,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Reads the version from the output of `claude --version`.
pub(crate) fn parse(output: &str) -> Option<Version> {
    let mut parts = output.split_whitespace().next()?.split('.');
    let mut number = || parts.next()?.parse().ok();
    let version = Version {
        major: number()?,
        minor: number()?,
        patch: number()?,
    };
    parts.next().is_none().then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_before_the_product_name() {
        assert_eq!(parse("2.1.311 (Claude Code)\n"), Some(MINIMUM));
    }

    #[test]
    fn orders_versions_by_number_not_by_text() {
        assert!(parse("2.1.400").unwrap() > MINIMUM);
        assert!(parse("2.1.99").unwrap() < MINIMUM);
        assert!(parse("10.0.0").unwrap() > MINIMUM);
    }

    #[test]
    fn rejects_output_that_is_not_a_version() {
        for output in ["", "claude", "2.1", "2.1.311.4", "2.1.x (Claude Code)"] {
            assert_eq!(parse(output), None, "{output:?}");
        }
    }
}
