use clap::Parser;
use sonic_rs::{JsonContainerTrait, JsonValueTrait};
use std::{
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
};
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    config_root: Option<PathBuf>,

    #[arg(short, long)]
    no_files: bool,

    #[arg(short = 'N', long)]
    no_dirs: bool,

    #[arg(short = 'F', long)]
    files_first: bool,
}
fn get_default_config_root() -> PathBuf {
    dirs::config_dir().expect("No config path!").join("Code")
}

fn main() -> Result<(), Error> {
    let Args {
        config_root,
        no_files,
        no_dirs,
        files_first,
    } = Args::parse();
    let config_root = config_root.unwrap_or_else(get_default_config_root);
    let storage_path = config_root.join("User/globalStorage/storage.json");
    let file = File::open(storage_path)?;
    let reader = BufReader::new(file);

    let value: sonic_rs::Value = sonic_rs::from_reader(reader)?;
    let items = value
        .as_object_get("lastKnownMenubarData")?
        .as_object_get("menus")?
        .as_object_get("File")?
        .as_object_get("items")?
        .as_array()
        .ok_or(Error::FailedUseAsArray)?;
    let recent = items
        .iter()
        .find(|item| {
            let Ok(id) = item.as_object_get("id") else {
                return false;
            };
            let Some(id) = id.as_str() else {
                return false;
            };
            id == "submenuitem.MenubarRecentMenu"
        })
        .unwrap();
    let uris = recent
        .as_object_get("submenu")?
        .as_object_get("items")?
        .as_array()
        .ok_or(Error::FailedUseAsArray)?
        .iter()
        .filter_map(|item| {
            let id = item.as_object_get("id").ok()?.as_str()?;
            if (no_files || id != "openRecentFile") && (no_dirs || id != "openRecentFolder") {
                return None;
            }
            if !item.get("enabled").and_then(|s| s.as_bool())? {
                return None;
            }
            let val = item
                .as_object_get("uri")
                .ok()?
                .as_object_get("path")
                .ok()?
                .as_str()?;
            let t = match id {
                "openRecentFile" => RecentEntryType::File,
                "openRecentFolder" => RecentEntryType::Dir,
                _ => unreachable!(),
            };
            Some(RecentEntry { t, val })
        })
        .collect::<Vec<_>>();

    let mut out = if files_first {
        let (files, dirs): (Vec<_>, Vec<_>) =
            uris.iter().partition(|e| e.t == RecentEntryType::File);
        files
            .into_iter()
            .chain(dirs)
            .map(|e| e.val)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        uris.into_iter()
            .map(|e| e.val)
            .collect::<Vec<_>>()
            .join("\n")
    };
    out.push('\0');
    println!("{out}");
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecentEntryType {
    File,
    Dir,
}

#[derive(Debug, Clone)]
struct RecentEntry<'a> {
    t: RecentEntryType,
    val: &'a str,
}
trait SonicRsValueExtensions {
    fn as_object_get<'a>(&'a self, key: &str) -> Result<&'a sonic_rs::Value, Error>;
}

impl SonicRsValueExtensions for sonic_rs::Value {
    fn as_object_get<'a>(&'a self, key: &str) -> Result<&'a sonic_rs::Value, Error> {
        let res = self
            .as_object()
            .ok_or(Error::FailedUseAsObject)?
            .get(&key)
            .ok_or(Error::FailedGettingKey(key.to_owned()))?;
        Ok(res)
    }
}

#[derive(Debug, Error)]
enum Error {
    #[error("Failed to use value as object.")]
    FailedUseAsObject,
    #[error("Failed to use value as array.")]
    FailedUseAsArray,
    #[error("Couldn't get value in object. Key: {}", .0)]
    FailedGettingKey(String),
    #[error("IO Error: {}", .0)]
    StdIo(#[from] io::Error),
    #[error("sonic-rs Error: {}", .0)]
    SonicRs(#[from] sonic_rs::error::Error),
}
