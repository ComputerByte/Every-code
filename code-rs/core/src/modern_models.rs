//! Public OpenAI API specifications verified 2026-10-09. OAuth transport
//! metadata comes from the separate Codex catalog; it is not an API contract.
//! Sources: https://developers.openai.com/api/docs/models/<model-id>
use crate::config_types::ReasoningEffort;

#[derive(Debug)]
pub struct ModernModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub supports_no_reasoning: bool,
}

pub const PUBLIC_CONTEXT_WINDOW: u64 = 1_050_000;
pub const PUBLIC_MAX_INPUT_TOKENS: u64 = 922_000;
pub const PUBLIC_MAX_OUTPUT_TOKENS: u64 = 128_000;
// Observed Codex backend limit, distinct from the public API window.
pub const CODEX_MAX_CONTEXT_WINDOW: u64 = 872_000;

pub const MODERN_MODELS: &[ModernModel] = &[
    ModernModel { id: "gpt-6.1-sol", display_name: "GPT-6.1 Sol", description: "Complex coding and professional work at lower cost than Astra.", supports_no_reasoning: false },
    ModernModel { id: "gpt-6-astra", display_name: "GPT-6 Astra", description: "Demanding reasoning, coding, and research.", supports_no_reasoning: false },
    ModernModel { id: "gpt-6-sol", display_name: "GPT-6 Sol", description: "Coding and agentic workflows.", supports_no_reasoning: true },
    ModernModel { id: "gpt-6-luna", display_name: "GPT-6 Luna", description: "Focused, high-volume tasks at low cost.", supports_no_reasoning: true },
    ModernModel { id: "gpt-5.6-sol", display_name: "GPT-5.6 Sol", description: "Flagship GPT-5.6 for complex professional work.", supports_no_reasoning: true },
    ModernModel { id: "gpt-5.6-terra", display_name: "GPT-5.6 Terra", description: "Balances intelligence and cost.", supports_no_reasoning: true },
    ModernModel { id: "gpt-5.6-luna", display_name: "GPT-5.6 Luna", description: "Cost-sensitive, high-volume workloads.", supports_no_reasoning: true },
];

const REASONING: &[ReasoningEffort] = &[
    ReasoningEffort::Low, ReasoningEffort::Medium, ReasoningEffort::High,
    ReasoningEffort::XHigh, ReasoningEffort::Max,
];
const OPTIONAL_REASONING: &[ReasoningEffort] = &[
    ReasoningEffort::Disabled, ReasoningEffort::Low, ReasoningEffort::Medium,
    ReasoningEffort::High, ReasoningEffort::XHigh, ReasoningEffort::Max,
];

pub fn modern_model(model: &str) -> Option<&'static ModernModel> {
    MODERN_MODELS.iter().find(|spec| spec.id.eq_ignore_ascii_case(model))
}

pub fn reasoning_efforts(model: &str) -> Option<&'static [ReasoningEffort]> {
    modern_model(model).map(|spec| {
        if spec.supports_no_reasoning { OPTIONAL_REASONING } else { REASONING }
    })
}
