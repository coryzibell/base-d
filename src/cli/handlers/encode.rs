use crate::cli::{args::EncodeArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _args: EncodeArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement encode handler")
}
