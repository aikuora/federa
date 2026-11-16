//! The sign-in against an engine defined here, on a paused clock, so the minutes a sign-in may take
//! pass at once.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use engine::{Agent, AgentSpec, Engine, Error, SignIn};
use kernel::Kernel;
use protocol::{Command, Event, Outcome, Reply, SignInStatus};
use tokio::time::Instant;

/// An engine whose sign-in completes at once, or never, and starts until it breaks.
#[derive(Default)]
struct Fake {
    completes: bool,
    broken: AtomicBool,
    signed_in: AtomicBool,
    stopped: AtomicUsize,
}

impl Engine for Fake {
    fn open(&self, _: AgentSpec) -> Result<Agent, Error> {
        unimplemented!("no agent opens in these tests")
    }

    fn sign_in_status(&self) -> Result<SignIn, Error> {
        Ok(if self.signed_in.load(Ordering::SeqCst) {
            SignIn::SignedIn
        } else {
            SignIn::SignedOut
        })
    }

    fn start_sign_in(&self) -> Result<(), Error> {
        if self.broken.load(Ordering::SeqCst) {
            return Err(Error::NotInstalled("claude".into()));
        }
        self.signed_in.store(self.completes, Ordering::SeqCst);
        Ok(())
    }

    fn stop_sign_in(&self) {
        self.stopped.fetch_add(1, Ordering::SeqCst);
    }
}

/// An engine that cannot run its binary any more.
struct Gone;

impl Engine for Gone {
    fn open(&self, _: AgentSpec) -> Result<Agent, Error> {
        unimplemented!("no agent opens in these tests")
    }

    fn sign_in_status(&self) -> Result<SignIn, Error> {
        Err(Error::NotInstalled("claude".into()))
    }

    fn start_sign_in(&self) -> Result<(), Error> {
        Err(Error::NotInstalled("claude".into()))
    }

    fn stop_sign_in(&self) {}
}

fn kernel(engine: Result<Arc<dyn Engine>, String>) -> Kernel {
    Kernel::new("ana@acme.dev".to_owned(), engine)
}

async fn status(kernel: &Kernel, command: Command) -> SignInStatus {
    match kernel.handle(command).await {
        Outcome::Done {
            reply: Reply::SignIn { status },
        } => status,
        outcome => panic!("{outcome:?}"),
    }
}

fn changed(status: SignInStatus) -> Event {
    Event::SignInChanged { status }
}

#[tokio::test(start_paused = true)]
async fn signs_in_and_announces_each_change() {
    let engine = Arc::new(Fake {
        completes: true,
        ..Fake::default()
    });
    let kernel = kernel(Ok(engine.clone()));
    let mut events = kernel.subscribe();
    assert_eq!(
        status(&kernel, Command::SignInStatus).await,
        SignInStatus::SignedOut
    );
    assert_eq!(
        status(&kernel, Command::StartSignIn).await,
        SignInStatus::SigningIn
    );
    // The provider already says signed in, but the kernel has not asked it yet.
    assert_eq!(
        status(&kernel, Command::SignInStatus).await,
        SignInStatus::SigningIn
    );
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SigningIn)));
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SignedIn)));
    assert_eq!(engine.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(
        status(&kernel, Command::SignInStatus).await,
        SignInStatus::SignedIn
    );
}

#[tokio::test(start_paused = true)]
async fn a_sign_in_not_completed_in_time_goes_back_to_signed_out() {
    let engine = Arc::new(Fake::default());
    let kernel = kernel(Ok(engine.clone()));
    let mut events = kernel.subscribe();
    let started = Instant::now();
    status(&kernel, Command::StartSignIn).await;
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SigningIn)));
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SignedOut)));
    assert!(started.elapsed() >= Duration::from_secs(5 * 60));
    assert_eq!(engine.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(
        status(&kernel, Command::SignInStatus).await,
        SignInStatus::SignedOut
    );
}

#[tokio::test(start_paused = true)]
async fn starting_again_gives_the_new_sign_in_all_its_time() {
    let engine = Arc::new(Fake::default());
    let kernel = kernel(Ok(engine.clone()));
    let mut events = kernel.subscribe();
    status(&kernel, Command::StartSignIn).await;
    tokio::time::sleep(Duration::from_secs(4 * 60)).await;
    let again = Instant::now();
    status(&kernel, Command::StartSignIn).await;
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SigningIn)));
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SigningIn)));
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SignedOut)));
    assert!(again.elapsed() >= Duration::from_secs(5 * 60));
    assert_eq!(engine.stopped.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn a_sign_in_that_fails_to_replace_another_announces_its_end() {
    let engine = Arc::new(Fake::default());
    let kernel = kernel(Ok(engine.clone()));
    let mut events = kernel.subscribe();
    status(&kernel, Command::StartSignIn).await;
    engine.broken.store(true, Ordering::SeqCst);
    let unavailable = SignInStatus::Unavailable {
        reason: Error::NotInstalled("claude".into()).to_string(),
    };
    assert_eq!(status(&kernel, Command::StartSignIn).await, unavailable);
    assert_eq!(events.recv().await, Ok(changed(SignInStatus::SigningIn)));
    assert_eq!(events.recv().await, Ok(changed(unavailable)));
    // The watch of the sign-in it replaced ended with it.
    tokio::time::sleep(Duration::from_secs(10 * 60)).await;
    assert!(events.try_recv().is_err());
    assert_eq!(engine.stopped.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn without_an_engine_answers_unavailable_with_the_reason() {
    let kernel = kernel(Err("claude could not be run; is it installed?".to_owned()));
    let mut events = kernel.subscribe();
    let unavailable = SignInStatus::Unavailable {
        reason: "claude could not be run; is it installed?".to_owned(),
    };
    for command in [Command::SignInStatus, Command::StartSignIn] {
        assert_eq!(status(&kernel, command).await, unavailable);
    }
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn an_engine_that_can_no_longer_run_answers_unavailable() {
    let kernel = kernel(Ok(Arc::new(Gone)));
    let unavailable = SignInStatus::Unavailable {
        reason: Error::NotInstalled("claude".into()).to_string(),
    };
    for command in [Command::SignInStatus, Command::StartSignIn] {
        assert_eq!(status(&kernel, command).await, unavailable);
    }
}
