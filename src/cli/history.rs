use crate::config::PigeonConfig;
use std::fs;
use toml_edit::{DocumentMut, value};

pub fn history_enable() -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();
    save_history_enabled(&config, true)
}

pub fn history_disable() -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();
    save_history_enabled(&config, false)
}

pub fn history_show() -> Result<(), Box<dyn std::error::Error>> {
    match fs::read_to_string(crate::history::path()) {
        Ok(history) => print!("{history}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

pub fn history_clear() -> Result<(), Box<dyn std::error::Error>> {
    crate::history::clear()?;
    Ok(())
}

fn save_history_enabled(
    config: &PigeonConfig,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(config.path())?;
    let mut document = source.parse::<DocumentMut>()?;

    document["history"]["enabled"] = value(enabled);

    fs::write(config.path(), document.to_string())?;
    Ok(())
}
