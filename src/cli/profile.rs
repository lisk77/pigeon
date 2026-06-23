use crate::config::PigeonConfig;
use std::fs;
use toml_edit::{DocumentMut, value};

pub fn profile_show() -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();
    println!("{}", config.profile.active);
    Ok(())
}

pub fn profile_list() -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();

    let mut profiles: Vec<String> = config.profiles.keys().cloned().collect();
    profiles.sort();

    for profile in profiles {
        println!("{}", profile);
    }

    Ok(())
}

pub fn profile_set(profile: String) -> Result<(), Box<dyn std::error::Error>> {
    let config = PigeonConfig::load_startup();

    if !config.profiles.contains_key(&profile) {
        return Err(format!("no profile named \"{}\" found", profile).into());
    }

    save_active_profile(&config, &profile)
}

fn save_active_profile(
    config: &PigeonConfig,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(config.path())?;
    let mut document = source.parse::<DocumentMut>()?;

    document["profile"]["active"] = value(name);

    fs::write(config.path(), document.to_string())?;
    Ok(())
}
