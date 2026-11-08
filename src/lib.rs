//! Official processing Nodes. Core/OS input and output adapters belong to the Host.
pub use diy_ime2_runtime::*;
pub mod compat;
pub mod nodes;
use node::{CoreError, Registry};
/// Register only the 48 previously registered non-system implementations.
/// The two prototypes remain opt-in.
pub fn register_nodes(r: &mut Registry) -> Result<(), CoreError> {
    r.register(compat::demo::Identity)?;
    r.register(compat::demo::Uppercase)?;
    r.register(compat::demo::SelectText)?;
    r.register(compat::demo::Branch)?;
    r.register(compat::initial_e2e::RomajiToHiragana)?;
    r.register(compat::initial_e2e::Segment)?;
    r.register(compat::initial_e2e::KanaKanjiCandidates)?;
    r.register(compat::initial_e2e::SelectCandidates)?;
    r.register(compat::initial_e2e::Stringify)?;
    r.register(nodes::romaji::RomajiToHiragana)?;
    r.register(nodes::stringify::Stringify)?;
    r.register(nodes::candidate_selection::SelectCandidates)?;
    r.register(nodes::dictionary_exact::DictionaryExact)?;
    nodes::experimental::register(r)
}

/// Experimental limited-data guest API, not the Core management API.
pub mod guest;
