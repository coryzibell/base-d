use crate::cli::{args::HashArgs, global::GlobalArgs};
use base_d::DictionaryRegistry;

pub fn handle(
    _args: HashArgs,
    _global: &GlobalArgs,
    _config: &DictionaryRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!("implement hash handler")
}
