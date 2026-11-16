use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use engine::{Engine, SignIn};
use protocol::{Event, SignInStatus};
use tokio::sync::broadcast;
use tokio::task::AbortHandle;

// A sign-in runs outside the runtime, in the person's browser, so the kernel learns it completed
// only by asking the engine again; it asks until the status is signed in or the time runs out,
// then ends the provider's sign-in and announces the status either way.

/// How often the kernel asks the engine while a sign-in runs.
const INTERVAL: Duration = Duration::from_secs(2);
/// How long the person has to complete a sign-in.
const LIMIT: Duration = Duration::from_secs(5 * 60);
/// How long a client waits for the engine to say whether it is signed in.
const ANSWER: Duration = Duration::from_secs(10);

type Watch = Arc<Mutex<Option<AbortHandle>>>;

/// The engine the agents run on, or why the runtime cannot run it, and the sign-in being watched.
pub(crate) struct Slot {
    engine: Result<Arc<dyn Engine>, String>,
    /// Starting again replaces the sign-in, so the watch of the one before stops with it, and
    /// never ends the new one when its own time runs out.
    watch: Watch,
}

impl Slot {
    pub(crate) fn new(engine: Result<Arc<dyn Engine>, String>) -> Self {
        Self {
            engine,
            watch: Arc::default(),
        }
    }

    /// The engine the agents run on, or why the runtime cannot run it.
    pub(crate) fn engine(&self) -> Result<&Arc<dyn Engine>, &str> {
        self.engine.as_ref().map_err(String::as_str)
    }

    /// The engine's sign-in status as a client sees it.
    pub(crate) async fn status(&self) -> SignInStatus {
        let engine = match &self.engine {
            Ok(engine) => engine.clone(),
            Err(reason) => return unavailable(reason),
        };
        if self.watching() {
            return SignInStatus::SigningIn;
        }
        // The client's connection waits for this answer, so an engine that never gives one must not
        // hold it.
        tokio::time::timeout(ANSWER, ask(engine))
            .await
            .unwrap_or_else(|_| {
                let seconds = ANSWER.as_secs();
                unavailable(&format!(
                    "the engine did not say within {seconds} s whether it is signed in"
                ))
            })
    }

    /// Starts the engine's official sign-in and watches it until it completes or times out,
    /// announcing each change on `events`. Answers the status right after starting.
    pub(crate) fn start(&self, events: broadcast::Sender<Event>) -> SignInStatus {
        let engine = match &self.engine {
            Ok(engine) => engine.clone(),
            Err(reason) => return unavailable(reason),
        };
        let mut watch = self.watch.lock().unwrap_or_else(PoisonError::into_inner);
        let replaced = watch.take().inspect(AbortHandle::abort).is_some();
        if let Err(error) = engine.start_sign_in() {
            let status = unavailable(&error.to_string());
            // Clients heard the sign-in it replaced start, and now hear it end.
            if replaced {
                announce(&events, status.clone());
            }
            return status;
        }
        announce(&events, SignInStatus::SigningIn);
        let watching = watch_until_done(engine, self.watch.clone(), events);
        *watch = Some(tokio::spawn(watching).abort_handle());
        SignInStatus::SigningIn
    }

    fn watching(&self) -> bool {
        let watch = self.watch.lock().unwrap_or_else(PoisonError::into_inner);
        watch.as_ref().is_some_and(|watch| !watch.is_finished())
    }
}

async fn watch_until_done(engine: Arc<dyn Engine>, watch: Watch, events: broadcast::Sender<Event>) {
    let asking = async {
        loop {
            tokio::time::sleep(INTERVAL).await;
            match ask(engine.clone()).await {
                SignInStatus::SignedOut => continue,
                status => return status,
            }
        }
    };
    let status = tokio::time::timeout(LIMIT, asking)
        .await
        .unwrap_or(SignInStatus::SignedOut);
    // An abort lands only where a task waits, so a watch that has stopped waiting checks under the
    // lock `start` holds that no newer sign-in replaced its own before ending it. It leaves the slot
    // first, so a client that heard the final status never hears signing in from `status` after.
    let mut current = watch.lock().unwrap_or_else(PoisonError::into_inner);
    if current.as_ref().map(AbortHandle::id) == Some(tokio::task::id()) {
        *current = None;
        engine.stop_sign_in();
        announce(&events, status);
    }
}

async fn ask(engine: Arc<dyn Engine>) -> SignInStatus {
    // The engine runs the provider's binary and waits for it, which would hold a worker thread.
    let asked = tokio::task::spawn_blocking(move || engine.sign_in_status())
        .await
        .unwrap_or_else(|error| std::panic::resume_unwind(error.into_panic()));
    match asked {
        Ok(SignIn::SignedIn) => SignInStatus::SignedIn,
        Ok(SignIn::SignedOut) => SignInStatus::SignedOut,
        Err(error) => unavailable(&error.to_string()),
    }
}

fn unavailable(reason: &str) -> SignInStatus {
    SignInStatus::Unavailable {
        reason: reason.to_owned(),
    }
}

fn announce(events: &broadcast::Sender<Event>, status: SignInStatus) {
    // Sending fails only when no client is connected, and then nobody is left to tell.
    let _ = events.send(Event::SignInChanged { status });
}
