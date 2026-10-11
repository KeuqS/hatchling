//! Channel types: identities, messages, and the trait.
//!
//! All identity in a [`Message`] is broker-asserted. The broker stamps
//! every inbound message with the authenticated peer identity; the agent
//! cannot spoof a sender. Never trust
//! a self-declared identity.

use serde::{Deserialize, Serialize};

/// Identifies a channel. Broker-assigned, never self-declared by the cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

impl ChannelId {
    /// The well-known local channel (stdio/loopback, v0.1).
    #[must_use]
    pub fn local() -> Self {
        Self(String::from("local"))
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifies a message sender. Broker-asserted from the authenticated
/// peer; the cell side cannot mint one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SenderId(pub String);

impl std::fmt::Display for SenderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A channel message: sender, channel, and text.
///
/// All three fields are broker-asserted. For inbound messages the broker
/// stamps the sender from the authenticated peer identity. For outbound
/// messages the agent supplies text (and the channel); the broker stamps
/// the sender before delivery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Who sent this, per the broker. Never self-declared by the cell.
    pub sender: SenderId,
    /// Which channel this arrived on / goes out on. Broker-asserted.
    pub channel: ChannelId,
    /// The message text. Untrusted: enters the loop below the system
    /// prompt, never above it.
    pub text: String,
}

/// The channel abstraction: one concrete channel per integration.
///
/// The broker owns the endpoints; the trait is what the broker drives
/// and what the session protocol mediates to the cell.
pub trait Channel: Send {
    /// The broker-assigned identity of this channel.
    fn channel_id(&self) -> ChannelId;

    /// Whether this channel authenticates to its backend with an
    /// account credential. A credentialed channel is only ever called
    /// with one (the broker resolves the cell's account handle to it);
    /// a credential-less channel, like the local one, never is.
    fn requires_credential(&self) -> bool {
        false
    }

    /// Receive the next inbound message, or `Ok(None)` if none is
    /// waiting. Blocking behavior is implementation-defined; the
    /// broker's session driver decides polling vs blocking.
    ///
    /// `credential` is the account secret the broker lends for this
    /// call, for authenticating to the backend only.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) when the channel backend fails.
    fn receive(
        &mut self,
        credential: Option<&crate::Credential<'_>>,
    ) -> Result<Option<Message>, crate::Error>;

    /// Deliver an outbound message. The `sender` field is ignored: the
    /// broker stamps the authenticated peer identity before delivery.
    ///
    /// `credential` is the account secret the broker lends for this
    /// call, for authenticating to the backend only.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) when the channel backend fails.
    fn send(
        &mut self,
        msg: Message,
        credential: Option<&crate::Credential<'_>>,
    ) -> Result<(), crate::Error>;
}

#[cfg(test)]
mod tests {
    use super::{ChannelId, SenderId};

    #[test]
    fn identities_display_as_their_inner_string() {
        assert_eq!(ChannelId(String::from("local")).to_string(), "local");
        assert_eq!(SenderId(String::from("uid:1000")).to_string(), "uid:1000");
    }
}
