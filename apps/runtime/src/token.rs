use std::fmt::Write;

use subtle::ConstantTimeEq;

/// The secret a client presents to connect. It never reaches a log.
pub(crate) struct Token(String);

impl Token {
    /// A new random token, drawn from the operating system.
    pub(crate) fn generate() -> Self {
        let mut bytes = [0u8; 32];
        // Without the operating system's randomness there is no token worth serving behind.
        getrandom::fill(&mut bytes).expect("the operating system provides random bytes");
        Self(bytes.iter().fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Compares in constant time, so the comparison does not leak how much of a guess was right.
    pub(crate) fn matches(&self, presented: &str) -> bool {
        self.0.as_bytes().ct_eq(presented.as_bytes()).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_itself() {
        let token = Token::generate();
        assert!(token.matches(token.as_str()));
        assert!(!token.matches(""));
        assert!(!token.matches(&token.as_str()[1..]));
        assert!(!token.matches(Token::generate().as_str()));
    }
}
