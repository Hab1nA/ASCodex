//! Developer context injected only after ASCodex verifies a Chief-issued StageBrief record.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

const START_MARKER: &str = "<ascodex_stage_brief>";
const END_MARKER: &str = "</ascodex_stage_brief>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ASCodexStageBrief {
    body: String,
}

impl ASCodexStageBrief {
    pub(crate) fn new(body: String) -> Self {
        Self { body }
    }
}

impl ContextualUserFragment for ASCodexStageBrief {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("ascodex.stage_brief".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (START_MARKER, END_MARKER)
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_as_a_separate_developer_fragment() {
        let brief = ASCodexStageBrief::new("bounded content".into());
        assert_eq!(brief.role(), "developer");
        assert!(brief.requires_separate_message());
        assert!(ASCodexStageBrief::matches_text(&brief.render()));
    }
}
