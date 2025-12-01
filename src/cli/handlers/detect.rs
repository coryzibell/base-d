use crate::cli::{args::DetectArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _args: DetectArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement detect handler")
}
