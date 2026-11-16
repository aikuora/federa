/// Whether the person is signed in to an engine's provider, where the engine's agents run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    SignedIn,
    SignedOut,
}
