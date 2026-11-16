use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{Error, Event};

// An agent is channels to a task the adapter runs, rather than a trait each adapter implements, so
// the kernel holds one concrete type whatever engine runs the agent.

/// A running agent.
#[derive(Debug)]
pub struct Agent {
    input: mpsc::Sender<String>,
    events: mpsc::Receiver<Event>,
    task: JoinHandle<()>,
}

impl Agent {
    /// For adapters: `task` reads each message from `input` and reports through `events`, and
    /// ends once `input` closes.
    pub fn new(
        input: mpsc::Sender<String>,
        events: mpsc::Receiver<Event>,
        task: JoinHandle<()>,
    ) -> Self {
        Self {
            input,
            events,
            task,
        }
    }

    pub async fn send(&self, message: String) -> Result<(), Error> {
        self.input.send(message).await.map_err(|_| Error::Closed)
    }

    /// The next event, or `None` once the agent ended.
    pub async fn next(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Lets the agent finish and waits until it has.
    pub async fn close(self) {
        let Self {
            input,
            events,
            task,
        } = self;
        drop(input);
        // Nobody reads the events any more, and a task waiting to send one would never finish.
        drop(events);
        if let Err(error) = task.await
            && error.is_panic()
        {
            std::panic::resume_unwind(error.into_panic());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // An adapter whose agent answers each message with its text and the end of the turn.
    fn echo() -> Agent {
        let (input, mut messages) = mpsc::channel::<String>(1);
        let (events, receiver) = mpsc::channel(1);
        let task = tokio::spawn(async move {
            while let Some(message) = messages.recv().await {
                let _ = events.send(Event::Text(message)).await;
                let _ = events.send(Event::TurnEnded).await;
            }
        });
        Agent::new(input, receiver, task)
    }

    #[tokio::test]
    async fn reports_the_events_of_each_message_in_order() {
        let mut agent = echo();
        agent.send("one".into()).await.unwrap();
        agent.send("two".into()).await.unwrap();
        assert_eq!(agent.next().await, Some(Event::Text("one".into())));
        assert_eq!(agent.next().await, Some(Event::TurnEnded));
        assert_eq!(agent.next().await, Some(Event::Text("two".into())));
        agent.close().await;
    }

    #[tokio::test]
    async fn refuses_a_message_once_its_task_ended() {
        let (input, messages) = mpsc::channel(1);
        let (_events, receiver) = mpsc::channel(1);
        drop(messages);
        let agent = Agent::new(input, receiver, tokio::spawn(async {}));
        assert!(matches!(agent.send("hi".into()).await, Err(Error::Closed)));
    }

    #[tokio::test]
    async fn closes_while_its_task_waits_to_report() {
        let agent = echo();
        agent.send("unread".into()).await.unwrap();
        agent.send("also unread".into()).await.unwrap();
        agent.close().await;
    }
}
