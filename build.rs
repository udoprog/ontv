use anyhow::{Result, Context};

fn main() -> Result<()> {
    if cfg!(target_os = "windows") {
        use winres::VersionInfo::*;

        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");

        if let Some((version, info)) = file_version() {
            res.set("FileVersion", &version);
            res.set("ProductVersion", &version);
            res.set_version_info(FILEVERSION, info);
            res.set_version_info(PRODUCTVERSION, info);
        }

        res.compile().context("compiling resorces")?;
    }

    Ok(())
}

fn file_version() -> Option<(String, u64)> {
    let version = match std::env::var("ONTV_FILE_VERSION") {
        Ok(version) => version,
        Err(_) => return None,
    };

    let mut info = 0u64;

    let mut it = version.split('.');

    info |= it.next()?.parse().unwrap_or(0) << 48;
    info |= it.next()?.parse().unwrap_or(0) << 32;
    info |= it.next()?.parse().unwrap_or(0) << 16;
    info |= match it.next() {
        Some(n) => n.parse().unwrap_or(0),
        None => 0,
    };

    Some((version, info))
}
