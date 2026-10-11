//! The capability probe: the one short request a model
//! provider sends before a task, to check that the model at the
//! endpoint really calls tools.
//!
//! The probe is a contract, not guidance: it hands the model one tool
//! and asks for a call to it. Its wording is prompt text, so it lives
//! here with the rest of the pack, and a change-detector test pins it.

/// The probe tool's name.
pub const PROBE_TOOL: &str = "hatchling_capability_probe";

/// The probe tool's description, as the model sees it.
pub const PROBE_TOOL_DESCRIPTION: &str =
    "Confirms that this model can call tools. Call it with ready set to true.";

/// The probe's one message.
pub const PROBE_INSTRUCTION: &str = "Capability check: call the hatchling_capability_probe tool with ready set to true. Answer with the tool call only, not text.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_text_is_pinned_and_names_its_tool() {
        assert_eq!(PROBE_TOOL, "hatchling_capability_probe");
        assert_eq!(
            PROBE_TOOL_DESCRIPTION,
            "Confirms that this model can call tools. Call it with ready set to true."
        );
        assert_eq!(
            PROBE_INSTRUCTION,
            "Capability check: call the hatchling_capability_probe tool with ready set to true. Answer with the tool call only, not text."
        );
        assert!(PROBE_INSTRUCTION.contains(PROBE_TOOL));
    }
}
