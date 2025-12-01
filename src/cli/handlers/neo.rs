use crate::cli::{args::NeoArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _args: NeoArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement neo handler")
}
