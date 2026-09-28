use crate::args::Configure;
use crate::config::Config;
use crate::output;
use concord_core::Result;
use serde_json::json;

pub fn run(config: &Config, command: Configure, json_output: bool) -> Result<()> {
    match command {
        Configure::Path => unreachable!("config path exits before loading the space"),
        Configure::Show => show(config, json_output),
    }
}

fn show(config: &Config, json_output: bool) -> Result<()> {
    output::value(
        json!({
            "domain_space_root": config.domain_space_root.display().to_string(),
            "home": config.home.display().to_string(),
            "releases": config.releases,
            "depot": config.depot,
        }),
        json_output,
    );
    Ok(())
}
