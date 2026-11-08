use crate::args::Configure;
use crate::config::Loaded;
use crate::output;
use concord_core::Result;
use serde_json::json;

pub fn run(loaded: &Loaded, command: Configure, json_output: bool) -> Result<()> {
    match command {
        Configure::Path => unreachable!("config path exits before loading the space"),
        Configure::Show => show(loaded, json_output),
    }
}

fn show(loaded: &Loaded, json_output: bool) -> Result<()> {
    let config = loaded.config();
    output::value(
        json!({
            "workspace": config.home.join("workspace").display().to_string(),
            "provenance": loaded.provenance(),
            "home": config.home.display().to_string(),
            "releases": config.releases,
            "depot": config.depot,
            "phase": config.phase,
        }),
        json_output,
    );
    Ok(())
}
