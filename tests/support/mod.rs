#![allow(dead_code, unused_imports)]
pub use diy_ime2_nodes::*;
mod runtime_input;
mod system_demo;
use system_demo::{config_keys, output};
mod system_e2e;
pub mod builtins {
    pub use super::system_demo::PacketSource;
    pub use diy_ime2_nodes::compat::demo::*;
    pub mod initial_e2e {
        pub use super::super::system_e2e::{DebugOutput, FixedInput};
        pub use diy_ime2_nodes::compat::initial_e2e::*;
    }
    pub fn register_demo_nodes(
        r: &mut super::node::Registry,
    ) -> Result<(), super::node::CoreError> {
        r.register(PacketSource)?;
        r.register(Identity)?;
        r.register(Uppercase)?;
        r.register(SelectText)?;
        r.register(Branch)
    }
    pub fn register_builtin_nodes(
        r: &mut super::node::Registry,
    ) -> Result<(), super::node::CoreError> {
        r.register(super::system_demo::PacketSource)?;
        r.register(super::system_e2e::FixedInput)?;
        r.register(super::system_e2e::DebugOutput)?;
        r.register(super::runtime_input::RuntimeTextInput)?;
        diy_ime2_nodes::register_nodes(r)
    }
}
