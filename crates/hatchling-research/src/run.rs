//! A research run: plan, gather, synthesize, report.
//!
//! Each phase is a run of the agent loop ([`hatchling_loop::Loop`]),
//! bounded, with its own system prompt and its own tools:
//!
//! 1. **Plan.** No tools. The model proposes a strategy and sources; the
//!    host reads the plan, and the user approves it or the run ends.
//! 2. **Gather.** Two tools, `fetch_source` and `amend_plan`, over the
//!    approved plan (see [`crate::gather`]).
//! 3. **Synthesize.** **No tools**, so nothing a source says can make the
//!    run act. Each read source is in the history as
//!    [`Message::Evidence`](hatchling_loop::Message::Evidence), in its
//!    untrusted-data envelope, below the system prompt and the task.
//! 4. **Report.** The host binds the draft to what was read
//!    ([`Report::from_reply`]).
//!
//! The run's deadline covers every phase. Past it, gathering stops, and
//! synthesis does not start.

use std::sync::{Mutex, PoisonError};

use hatchling_loop::{Loop, LoopBudget, Model, ToolRegistry};
use hatchling_prompt::research::{Limits, Phase, phase_prompt};

use crate::bounds::{Bounds, MAX_SOURCES};
use crate::gather::{
    AmendPlan, Approver, Clock, Decision, Fetch, FetchSource, Fetcher, Gather, Gathered,
    SourceRecord,
};
use crate::plan::{Plan, PlanError};
use crate::report::{Report, ReportError};

/// A research run's parts.
pub struct Research<'a> {
    /// The model behind every phase.
    pub model: &'a dyn Model,
    /// How sources are fetched: through the egress proxy, in the agent.
    pub fetcher: &'a dyn Fetcher,
    /// The user, who approves the plan and each amendment.
    pub approver: &'a dyn Approver,
    /// The time.
    pub clock: &'a dyn Clock,
    /// The run's bounds.
    pub bounds: Bounds,
}

impl std::fmt::Debug for Research<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Research")
            .field("bounds", &self.bounds)
            .finish_non_exhaustive()
    }
}

/// A finished run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// What was gathered, under which plan.
    pub gathered: Gathered,
    /// The report.
    pub report: Report,
}

impl Outcome {
    /// The report as the document builder's inputs.
    #[must_use]
    pub fn inputs(&self) -> Vec<(&'static str, String)> {
        self.report.inputs(&self.gathered)
    }
}

/// Why a run did not finish.
#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    /// The proposed plan was unusable.
    #[error("the plan was unusable: {0}")]
    Plan(#[from] PlanError),
    /// The user declined the plan. Nothing was fetched.
    #[error("the user declined the plan: {reason}")]
    PlanDeclined {
        /// Why, in the user's words.
        reason: String,
    },
    /// The time bound was reached before synthesis.
    #[error("the time bound was reached before synthesis")]
    OutOfTime {
        /// What had been gathered.
        gathered: Box<Gathered>,
    },
    /// A phase's loop failed: the model failed, or broke the tool
    /// contract.
    #[error("a research phase failed: {0}")]
    Loop(#[from] hatchling_loop::Error),
    /// The drafted report was unusable.
    #[error("the report was unusable: {0}")]
    Report(#[from] ReportError),
}

impl Research<'_> {
    /// Runs the research for `question`.
    ///
    /// # Errors
    ///
    /// Returns why the run did not finish.
    pub fn run(&self, question: &str) -> Result<Outcome, ResearchError> {
        let deadline = self.clock.now().saturating_add(self.bounds.secs());
        let plan = self.plan(question)?;
        if let Decision::Decline { reason } = self.approver.review_plan(&plan) {
            return Err(ResearchError::PlanDeclined { reason });
        }
        let gathered = self.gather(plan, deadline)?;
        if self.clock.now() >= deadline {
            return Err(ResearchError::OutOfTime {
                gathered: Box::new(gathered),
            });
        }
        let draft = self.synthesize(&gathered)?;
        let report = Report::from_reply(&draft, &gathered)?;
        Ok(Outcome { gathered, report })
    }

    fn plan(&self, question: &str) -> Result<Plan, ResearchError> {
        let tools = ToolRegistry::new();
        let reply = Loop::new(&tools, Phase::Plan.kind())
            .with_system_prompt(phase_prompt(Phase::Plan, limits(self.bounds)))
            .with_budget(LoopBudget { max_steps: 1 })
            .run(self.model, question.to_owned())?;
        Ok(Plan::from_reply(question, &reply.answer)?)
    }

    fn gather(&self, plan: Plan, deadline: i64) -> Result<Gathered, ResearchError> {
        let request = plan.render();
        let state = Mutex::new(Gather::new(
            plan,
            self.bounds,
            deadline,
            self.fetcher,
            self.approver,
            self.clock,
        ));
        let ran = {
            let mut tools = ToolRegistry::new();
            tools.register(Box::new(FetchSource::new(&state)));
            tools.register(Box::new(AmendPlan::new(&state)));
            Loop::new(&tools, Phase::Gather.kind())
                .with_system_prompt(phase_prompt(Phase::Gather, limits(self.bounds)))
                .with_budget(LoopBudget {
                    max_steps: gather_steps(self.bounds),
                })
                .run(self.model, request)
        };
        let step_bound = match ran {
            Ok(_) => false,
            Err(hatchling_loop::Error::BudgetExhausted { .. }) => true,
            Err(error) => return Err(error.into()),
        };
        Ok(state
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .finish(step_bound))
    }

    fn synthesize(&self, gathered: &Gathered) -> Result<String, ResearchError> {
        // No tools: whatever a source says, synthesis cannot act on it.
        let tools = ToolRegistry::new();
        let evidence = gathered.records.iter().filter_map(evidence).collect();
        let reply = Loop::new(&tools, Phase::Synthesize.kind())
            .with_system_prompt(phase_prompt(Phase::Synthesize, limits(self.bounds)))
            .with_budget(LoopBudget { max_steps: 1 })
            .run_with_evidence(self.model, gathered.plan.render(), evidence)?;
        Ok(reply.answer)
    }
}

/// The run's limits, as its prompts state them.
const fn limits(bounds: Bounds) -> Limits {
    Limits {
        planned: MAX_SOURCES,
        read: bounds.sources(),
        secs: bounds.secs(),
    }
}

/// The gather phase's model turns: two per source (a fetch, and an
/// amendment or a retry), and two more to finish.
fn gather_steps(bounds: Bounds) -> u32 {
    u32::try_from(bounds.sources().saturating_mul(2).saturating_add(2)).unwrap_or(u32::MAX)
}

/// A read source, in its untrusted-data envelope.
fn evidence(record: &SourceRecord) -> Option<String> {
    let Fetch::Read { sha256, text, .. } = &record.fetch else {
        return None;
    };
    Some(hatchling_prompt::wrap_source(
        &record.id,
        record.url.as_str(),
        &crate::timestamp(record.fetched_at),
        sha256,
        text,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gather::{FetchFailure, Fetched};
    use crate::url::SourceUrl;

    /// Fetches nothing, approves nothing, and stops the clock.
    struct Nothing;

    impl Fetcher for Nothing {
        fn fetch(&self, _url: &SourceUrl) -> Result<Fetched, FetchFailure> {
            Err(FetchFailure::Failed)
        }
    }

    impl Approver for Nothing {
        fn review_plan(&self, _plan: &Plan) -> Decision {
            Decision::Approve
        }

        fn review_amendment(&self, _plan: &Plan, _url: &SourceUrl, _why: &str) -> Decision {
            Decision::Approve
        }
    }

    impl Clock for Nothing {
        fn now(&self) -> i64 {
            0
        }
    }

    #[test]
    fn a_run_shows_its_bounds_and_nothing_it_borrows() {
        let model = hatchling_loop::FakeModel::new(Vec::new());
        let research = Research {
            model: &model,
            fetcher: &Nothing,
            approver: &Nothing,
            clock: &Nothing,
            bounds: Bounds::standard(),
        };
        assert_eq!(
            format!("{research:?}"),
            "Research { bounds: Bounds { sources: 8, secs: 900 }, .. }"
        );
        assert_eq!(gather_steps(Bounds::standard()), 18);
        let limits = limits(Bounds::new(3, 60).unwrap());
        assert_eq!((limits.planned, limits.read, limits.secs), (32, 3, 60));
    }
}
