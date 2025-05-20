use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
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
    with_files: bool,

    #[arg(short = 'W', long)]
    with_dirs: bool,

    #[arg(short = 'd', long)]
    order: RecentOrder,
}
fn get_default_config_root() -> PathBuf {
    dirs::config_dir().expect("No config path!").join("Code")
}

#[derive(Debug, Clone, Default, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RecentOrder {
    #[default]
    Unchanged,
    FilesFirst,
    DirsFirst,
}

fn main() -> Result<(), Error> {
    let Args {
        config_root,
        with_files,
        with_dirs,
        order,
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
            let keep_id =
                with_files && id == "openRecentFile" || with_dirs && id == "openRecentFolder";
            if !keep_id {
                return None;
            }
            let is_enabled = item.get("enabled").and_then(|s| s.as_bool())?;
            if !is_enabled {
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

    let mut out = match order {
        RecentOrder::Unchanged => uris
            .into_iter()
            .map(|e| e.val)
            .collect::<Vec<_>>()
            .join("\n"),
        RecentOrder::FilesFirst | RecentOrder::DirsFirst => {
            let (first, second): (Vec<_>, Vec<_>) = uris.iter().partition(|e| {
                // want_file xnor is_file
                !((order == RecentOrder::FilesFirst) ^ (e.t == RecentEntryType::File))
            });
            first
                .into_iter()
                .chain(second)
                .map(|e| e.val)
                .collect::<Vec<_>>()
                .join("\n")
        }
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
