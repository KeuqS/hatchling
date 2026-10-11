//! A scripted model for tests.
//!
//! `FakeModel` plays a programmed queue of actions and records the
//! history it was shown each turn, so tests pin the loop's behavior
//! (history accumulation, budget enforcement, error feedback) by
//! assertion rather than by vibes.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::Error;
use crate::model::{Message, Model, ModelAction};
use crate::tool::ToolDescription;

/// A model with a programmed action queue.
///
/// Each `next_action` pops the front of the queue. An empty queue is
/// [`Error::ModelFailed`] — the test scripted too few actions.
pub struct FakeModel {
    actions: Mutex<VecDeque<ModelAction>>,
    seen: Mutex<Vec<Vec<Message>>>,
}

impl FakeModel {
    /// Build from a queue of actions, in call order.
    #[must_use]
    pub fn new(actions: Vec<ModelAction>) -> Self {
        Self {
            actions: Mutex::new(actions.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// The histories the model was shown, one per `next_action`
    /// call, in order. Tests assert the loop feeds back tool
    /// results correctly.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned. `FakeModel` is
    /// test-only and single-threaded; a poisoned mutex means a test
    /// bug, and panicking surfaces it instead of deadlocking.
    #[must_use]
    pub fn seen_histories(&self) -> Vec<Vec<Message>> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Model for FakeModel {
    fn next_action(
        &self,
        messages: &[Message],
        _tools: &[ToolDescription],
    ) -> Result<ModelAction, Error> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(messages.to_vec());
        self.actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
            .ok_or_else(|| Error::ModelFailed(String::from("fake model ran out of actions")))
    }
}

impl std::fmt::Debug for FakeModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeModel").finish_non_exhaustive()
    }
}
