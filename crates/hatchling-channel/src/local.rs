//! The local channel: stdio/loopback for v0.1.
//!
//! The proof that the [`Channel`] abstraction works. Real local use
//! reads inbound messages from stdin (one JSON [`Message`] per line)
//! and writes outbound messages to stdout (same framing). Tests use
//! the in-memory queue form. The broker's local channel is backed by a
//! [`LocalHub`] its ingress feeds.

use std::collections::VecDeque;
use std::io::{BufRead, Write};

use crate::{Channel, ChannelId, Credential, Error, LocalHub, Message};

/// A local channel: stdin/stdout in production, in-memory in tests.
///
/// Inbound: each line on stdin is a JSON [`Message`]. The broker stamps
/// the sender from the authenticated peer before handing it to the
/// agent — a stdin line claiming a sender is still broker-stamped.
/// Outbound: each [`Message`] is written as one JSON line on stdout.
pub struct LocalChannel {
    inbound: VecDeque<Message>,
    outbound: Vec<Message>,
    use_stdio: bool,
    /// The broker's ingress hub, when this channel is backed by one:
    /// receive takes from its queue and send delivers to its client.
    hub: Option<LocalHub>,
    /// The session this channel delivers for: the output gate's scope.
    scope: String,
}

impl LocalChannel {
    /// An in-memory local channel, for tests and the fixture broker.
    #[must_use]
    #[allow(clippy::missing_const_for_fn, reason = "VecDeque::new is not const")]
    pub fn in_memory() -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: Vec::new(),
            use_stdio: false,
            hub: None,
            scope: String::new(),
        }
    }

    /// A local channel backed by the broker's ingress `hub`: messages
    /// the local user injected arrive here, and replies go back to
    /// their client, through the hub's output gate, in `scope` (the
    /// session's own).
    #[must_use]
    #[allow(clippy::missing_const_for_fn, reason = "VecDeque::new is not const")]
    pub fn with_hub(hub: LocalHub, scope: String) -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: Vec::new(),
            use_stdio: false,
            hub: Some(hub),
            scope,
        }
    }

    /// The stdio-backed local channel, for real local use.
    #[must_use]
    #[allow(clippy::missing_const_for_fn, reason = "VecDeque::new is not const")]
    pub fn stdio() -> Self {
        Self {
            inbound: VecDeque::new(),
            outbound: Vec::new(),
            use_stdio: true,
            hub: None,
            scope: String::new(),
        }
    }

    /// Queue an inbound message (test/fixture use).
    pub fn push_inbound(&mut self, msg: Message) {
        self.inbound.push_back(msg);
    }

    /// Drain the outbound messages (test/fixture use).
    #[must_use]
    pub fn drain_outbound(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.outbound)
    }
}

impl Channel for LocalChannel {
    fn channel_id(&self) -> ChannelId {
        ChannelId::local()
    }

    fn receive(&mut self, _credential: Option<&Credential<'_>>) -> Result<Option<Message>, Error> {
        if let Some(msg) = self.inbound.pop_front() {
            return Ok(Some(msg));
        }
        if let Some(hub) = &self.hub {
            return Ok(hub.take());
        }
        if !self.use_stdio {
            return Ok(None);
        }
        let stdin = std::io::stdin();
        let mut line = String::new();
        // The lock guard is bound before the match so it drops at the
        // end of the statement, not at the end of the match arm.
        let mut locked = stdin.lock();
        match locked.read_line(&mut line) {
            Ok(0) => Ok(None),
            Ok(_) => {
                let msg: Message = serde_json::from_str(line.trim())
                    .map_err(|e| Error::Backend(format!("invalid message on stdin: {e}")))?;
                Ok(Some(msg))
            }
            Err(e) => Err(Error::Backend(format!("stdin read failed: {e}"))),
        }
    }

    fn send(&mut self, msg: Message, _credential: Option<&Credential<'_>>) -> Result<(), Error> {
        // The sender is broker-stamped before this is called; the
        // channel delivers what the broker authorized.
        let stamped = Message {
            sender: msg.sender,
            channel: ChannelId::local(),
            text: msg.text,
        };
        if let Some(hub) = &self.hub {
            return hub.deliver(&stamped, &self.scope);
        }
        if self.use_stdio {
            let line = serde_json::to_string(&stamped)
                .map_err(|e| Error::Backend(format!("message encode failed: {e}")))?;
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "{line}")
                .map_err(|e| Error::Backend(format!("stdout write failed: {e}")))?;
            out.flush()
                .map_err(|e| Error::Backend(format!("stdout flush failed: {e}")))?;
        } else {
            self.outbound.push(stamped);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SenderId;

    /// Pins the stdio/in-memory split in `receive`: with nothing queued,
    /// only the stdio channel reads stdin. Stdin is a pipe holding one
    /// message line for the duration (restored after), so an in-memory
    /// channel that read it, or a stdio channel that did not, shows.
    #[test]
    fn only_the_stdio_channel_reads_stdin() {
        let line = serde_json::to_string(&Message {
            sender: SenderId(String::from("uid:1")),
            channel: ChannelId::local(),
            text: String::from("from stdin"),
        })
        .unwrap();
        let bytes = format!("{line}\n");
        let (read_end, write_end) = rustix::pipe::pipe().unwrap();
        assert_eq!(
            rustix::io::write(&write_end, bytes.as_bytes()).unwrap(),
            bytes.len()
        );
        drop(write_end);
        let saved = rustix::io::dup(rustix::stdio::stdin()).unwrap();
        rustix::stdio::dup2_stdin(&read_end).unwrap();
        let in_memory = LocalChannel::in_memory().receive(None);
        let stdio = LocalChannel::stdio().receive(None);
        rustix::stdio::dup2_stdin(&saved).unwrap();
        assert_eq!(in_memory.unwrap(), None, "the in-memory channel read stdin");
        assert_eq!(
            stdio.unwrap().map(|msg| msg.text),
            Some(String::from("from stdin"))
        );
    }

    fn msg(text: &str) -> Message {
        Message {
            sender: SenderId(String::from("uid:1000")),
            channel: ChannelId::local(),
            text: text.to_string(),
        }
    }

    #[test]
    fn in_memory_channel_round_trips() {
        let mut ch = LocalChannel::in_memory();
        assert_eq!(ch.channel_id(), ChannelId::local());
        assert!(
            !ch.requires_credential(),
            "the local channel is credential-less"
        );
        assert!(ch.receive(None).unwrap().is_none());

        ch.push_inbound(msg("hello"));
        let got = ch.receive(None).unwrap().unwrap();
        assert_eq!(got.text, "hello");
        assert_eq!(got.sender.0, "uid:1000");
        assert!(ch.receive(None).unwrap().is_none());
    }

    #[test]
    fn a_hub_backed_channel_receives_from_and_replies_through_the_hub() {
        let hub = LocalHub::new(crate::OutputGate::disabled());
        let mut ch = LocalChannel::with_hub(hub.clone(), String::from("session:a"));
        assert!(ch.receive(None).unwrap().is_none(), "empty hub, no message");
        assert_eq!(
            hub.inject(msg("from the user")),
            crate::IngressEvent::Queued
        );
        assert_eq!(
            ch.receive(None).unwrap().map(|m| m.text),
            Some(String::from("from the user"))
        );
        assert_eq!(hub.queued(), 0);

        // No client attached: the reply has nowhere to go.
        assert!(ch.send(msg("lost"), None).is_err());
        let delivered = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_delivered = std::sync::Arc::clone(&delivered);
        let _ = hub.attach(Box::new(move |m: &crate::Cleared| {
            sink_delivered.lock().unwrap().push(m.message().clone());
            Ok(())
        }));
        let mut claimed = msg("reply");
        claimed.channel = ChannelId(String::from("elsewhere"));
        ch.send(claimed, None).unwrap();
        let delivered = delivered.lock().unwrap().clone();
        assert_eq!(delivered.len(), 1);
        let first = delivered.first().expect("one reply");
        assert_eq!(first.text, "reply");
        assert_eq!(first.channel, ChannelId::local());
        assert!(ch.drain_outbound().is_empty(), "the hub, not the buffer");
    }

    #[test]
    fn send_records_outbound_with_local_channel_id() {
        let mut ch = LocalChannel::in_memory();
        ch.send(msg("reply"), None).unwrap();
        let out = ch.drain_outbound();
        assert_eq!(out.len(), 1);
        let first = out.first().expect("one outbound message");
        assert_eq!(first.text, "reply");
        assert_eq!(first.channel, ChannelId::local());
    }
}
