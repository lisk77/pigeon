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
        return Err(std::io::Error::other(format!("{path:?} is not a regular file")).into());
    }

    PigeonConfig::load(&path)?;

    if is_default_config_path(&path) {
        remove_path_pointer()?;
    } else {
        write_path_pointer(&path)?;
    }

    Ok(())
}

fn is_default_config_path(path: &std::path::Path) -> bool {
    match PigeonConfig::default_path().canonicalize() {
        Ok(default_path) => path == default_path,
        Err(_) => false,
    }
}

fn remove_path_pointer() -> std::io::Result<()> {
    match std::fs::remove_file(PigeonConfig::path_pointer_file()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn write_path_pointer(path: &std::path::Path) -> std::io::Result<()> {
    let pointer = PigeonConfig::path_pointer_file();
    let parent = pointer.parent().ok_or_else(|| {
        std::io::Error::other(format!(
            "config path pointer has no parent directory: {pointer:?}"
        ))
    })?;

    std::fs::create_dir_all(parent)?;

    std::fs::write(pointer, path.to_string_lossy().as_bytes())
}
