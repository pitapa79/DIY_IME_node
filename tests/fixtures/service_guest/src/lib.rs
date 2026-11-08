//! Test-only consumer of the public service helper and export macro.
#[path = "../../../support/service.rs"]
mod canary;
use canary::Canary;
diy_ime2_nodes::export_service_guest!(Canary::default());
