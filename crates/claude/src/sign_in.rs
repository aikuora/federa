// Agents run on a configuration folder of their own, so signing in from the person's terminal
// never reaches them; only the app starts the official sign-in on that folder.

/// What the person does when an agent finds its configuration folder not signed in.
pub(crate) const HINT: &str =
    "connect Claude from the app, which opens Claude's official sign-in for its own configuration";
