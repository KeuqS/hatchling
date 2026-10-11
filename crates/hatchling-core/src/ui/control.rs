//! Who is driving: the control state of a surface the agent and
//! the user can both drive, such as a browser session.
//!
//! Control is never ambiguous. A surface is driven by the agent or by the
//! user, or it is changing hands, and nothing is driven while it changes
//! hands. It moves only by the four [`Handoff`]s, and [`Control::after`]
//! is the whole transition table. The schema carries the table, so the
//! broker and the UI apply exactly the same rules. A handoff the table
//! does not allow is not a transition at all.
//!
//! The pattern is generic: later shared surfaces (a terminal, a device)
//! reuse these types rather than invent their own.

use serde::{Deserialize, Serialize};

/// Who controls a shared surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// The agent drives.
    Agent,
    /// The user drives; the agent is paused.
    User,
    /// Control is passing to the user. Nobody drives.
    ToUser,
    /// Control is passing back to the agent. Nobody drives.
    ToAgent,
}

/// Who did something on a shared surface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// The agent.
    Agent,
    /// The user.
    User,
}

/// One step in handing control over.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Handoff {
    /// The user asked for control.
    Takeover,
    /// The user handed control back.
    HandBack,
    /// The user has control: the agent has stopped.
    Granted,
    /// The agent has control again, having looked at the surface afresh.
    Resumed,
}

impl Control {
    /// Every control state, in declaration order.
    pub const ALL: [Self; 4] = [Self::Agent, Self::User, Self::ToUser, Self::ToAgent];

    /// The state after `handoff`, or `None` if the handoff is not allowed
    /// from here.
    ///
    /// The user asks for control only from the agent, and hands it back
    /// only from themselves. A change of hands settles either way, so a
    /// broker that cannot hand over says so by resuming the agent.
    #[must_use]
    pub const fn after(self, handoff: Handoff) -> Option<Self> {
        match self {
            Self::Agent => match handoff {
                Handoff::Takeover => Some(Self::ToUser),
                Handoff::HandBack | Handoff::Granted | Handoff::Resumed => None,
            },
            Self::User => match handoff {
                Handoff::HandBack => Some(Self::ToAgent),
                Handoff::Takeover | Handoff::Granted | Handoff::Resumed => None,
            },
            Self::ToUser | Self::ToAgent => match handoff {
                Handoff::Granted => Some(Self::User),
                Handoff::Resumed => Some(Self::Agent),
                Handoff::Takeover | Handoff::HandBack => None,
            },
        }
    }

    /// Whether `actor` drives in this state. Nobody drives while control
    /// changes hands.
    #[must_use]
    pub const fn drives(self, actor: Actor) -> bool {
        matches!(
            (self, actor),
            (Self::Agent, Actor::Agent) | (Self::User, Actor::User)
        )
    }
}

impl Handoff {
    /// Every handoff, in declaration order.
    pub const ALL: [Self; 4] = [Self::Takeover, Self::HandBack, Self::Granted, Self::Resumed];
}

/// The whole transition table, as `(from, handoff, to)`: every allowed
/// handoff and nothing else. This is what the schema carries.
#[must_use]
pub fn transitions() -> Vec<(Control, Handoff, Control)> {
    Control::ALL
        .iter()
        .flat_map(|&from| {
            Handoff::ALL
                .iter()
                .filter_map(move |&handoff| from.after(handoff).map(|to| (from, handoff, to)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_handoff_from_every_state_is_pinned() {
        use Control::{Agent, ToAgent, ToUser, User};
        use Handoff::{Granted, HandBack, Resumed, Takeover};
        let expected = [
            (Agent, Takeover, Some(ToUser)),
            (Agent, HandBack, None),
            (Agent, Granted, None),
            (Agent, Resumed, None),
            (User, Takeover, None),
            (User, HandBack, Some(ToAgent)),
            (User, Granted, None),
            (User, Resumed, None),
            (ToUser, Takeover, None),
            (ToUser, HandBack, None),
            (ToUser, Granted, Some(User)),
            (ToUser, Resumed, Some(Agent)),
            (ToAgent, Takeover, None),
            (ToAgent, HandBack, None),
            (ToAgent, Granted, Some(User)),
            (ToAgent, Resumed, Some(Agent)),
        ];
        for (from, handoff, to) in expected {
            assert_eq!(from.after(handoff), to, "{from:?} + {handoff:?}");
        }
    }

    #[test]
    fn only_the_driver_drives_and_nobody_drives_in_transition() {
        assert!(Control::Agent.drives(Actor::Agent));
        assert!(!Control::Agent.drives(Actor::User));
        assert!(Control::User.drives(Actor::User));
        assert!(!Control::User.drives(Actor::Agent));
        for state in [Control::ToUser, Control::ToAgent] {
            assert!(!state.drives(Actor::Agent), "{state:?}");
            assert!(!state.drives(Actor::User), "{state:?}");
        }
    }

    #[test]
    fn the_table_is_exactly_the_allowed_handoffs_in_order() {
        use Control::{Agent, ToAgent, ToUser, User};
        use Handoff::{Granted, HandBack, Resumed, Takeover};
        assert_eq!(
            transitions(),
            vec![
                (Agent, Takeover, ToUser),
                (User, HandBack, ToAgent),
                (ToUser, Granted, User),
                (ToUser, Resumed, Agent),
                (ToAgent, Granted, User),
                (ToAgent, Resumed, Agent),
            ]
        );
    }

    #[test]
    fn the_wire_names_are_pinned() {
        assert_eq!(
            serde_json::to_string(&Control::ALL).unwrap(),
            r#"["agent","user","to_user","to_agent"]"#
        );
        assert_eq!(
            serde_json::to_string(&Handoff::ALL).unwrap(),
            r#"["takeover","hand_back","granted","resumed"]"#
        );
        assert_eq!(
            serde_json::to_string(&[Actor::Agent, Actor::User]).unwrap(),
            r#"["agent","user"]"#
        );
    }
}
