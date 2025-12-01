use crate::cli::{args::DecodeArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _args: DecodeArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement decode handler")
}
