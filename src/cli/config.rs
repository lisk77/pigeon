use crate::config::PigeonConfig;
use std::path::PathBuf;

pub fn config_default() -> Result<(), Box<dyn std::error::Error>> {
    print!("{}", PigeonConfig::default_toml()?);
    Ok(())
}

pub fn config_path() -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();

    println!("{:?}", config.path);

    Ok(())
}

pub fn config_set_path(path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;

    if !path.is_file() {
        return Err(format!("{:?} is not a regular file", path).into());
    }

    PigeonConfig::load(&path)?;

    Ok(())
}
