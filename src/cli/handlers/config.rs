use crate::cli::{args::ConfigAction, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _action: ConfigAction,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement config handler")
}
