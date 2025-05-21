use anyhow::anyhow;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use sonic_rs::{JsonContainerTrait, JsonValueTrait};
use std::{
    fs::{self, DirEntry, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
    string::FromUtf8Error,
    time::{Duration, SystemTime},
    usize,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[arg(short, long)]
    config_root: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

fn get_default_config_root() -> PathBuf {
    dirs::config_dir().expect("No config path!").join("Code")
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    Recent {
        #[arg(short = 'w', long)]
        with_files: bool,
        #[arg(short = 'W', long)]
        with_dirs: bool,
        #[arg(short, long)]
        all: bool,
        #[arg(short = 'd', long, default_value_t, value_enum)]
        order: RecentOrder,
    },
    Workspaces {
        #[arg(short = 'x', long)]
        max_age_days: Option<u32>,
        #[arg(short, long)]
        limit: Option<usize>,
    },
}

#[derive(Debug, Clone, Default, ValueEnum, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RecentOrder {
    #[default]
    Unchanged,
    FilesFirst,
    DirsFirst,
}

fn main() -> anyhow::Result<()> {
    let Args {
        config_root,
        command,
    } = Args::parse();
    let config_root = config_root.unwrap_or_else(get_default_config_root);

    match command {
        Command::Recent {
            with_files,
            with_dirs,
            all,
            order,
        } => {
            collect_items_in_menu_settings(
                config_root,
                all || with_files,
                all || with_dirs,
                order,
            )?;
        }
        Command::Workspaces {
            max_age_days,
            limit,
        } => {
            collect_items_in_workspaces(config_root, max_age_days, limit)?;
        }
    }
    Ok(())
}

fn collect_items_in_workspaces(
    mut storage_path: PathBuf,
    max_age_days: Option<u32>,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    storage_path.push("User/workspaceStorage");

    let min_system_time = if let Some(max_age_days) = max_age_days {
        const NUM_SECONDS_IN_DAY: u64 = 86400;
        Some(
            SystemTime::now()
                .checked_sub(Duration::from_secs(
                    (max_age_days as u64) * NUM_SECONDS_IN_DAY,
                ))
                .ok_or_else(|| anyhow!("`max-age-days` too big"))?,
        )
    } else {
        Default::default()
    };

    let mut entries = fs::read_dir(&storage_path)?
        .filter_map(|entry| match prepare_dir_entry(entry) {
            Err(err) => {
                eprintln!("Error at: {}", &storage_path.as_os_str().to_string_lossy());
                eprintln!("Error reading workspace entry! {err}");
                None
            }
            Ok(entry) => {
                let is_recent_enough = min_system_time
                    .map(|min_system_time| entry.last_modified_at > min_system_time)
                    .unwrap_or(true);
                is_recent_enough.then_some(entry)
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|e1, e2| e1.last_modified_at.cmp(&e2.last_modified_at).reverse());

    let limit = limit.unwrap_or(usize::MAX);

    for FolderEntry { path, .. } in entries.into_iter().take(limit) {
        let path = path.join("workspace.json");
        if let Err(err) = digest_dir_entry(&path) {
            eprintln!("Error with file: {}", &path.as_os_str().to_string_lossy());
            eprintln!("Error digesting workspace entry! {err}");
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct FolderEntry {
    path: PathBuf,
    last_modified_at: SystemTime,
}

fn digest_dir_entry(path: &Path) -> anyhow::Result<()> {
    let mut file = File::open(path)?;
    let mut v: Vec<u8> = Vec::new();
    file.read_to_end(&mut v)?;
    let value: sonic_rs::Value = sonic_rs::from_slice(&v)?;

    let Ok(field) = value.as_object_get("folder") else {
        return Ok(());
    };
    let val = field
        .as_str()
        .ok_or_else(|| anyhow!("Failed using field in json as a string!"))?;

    let val = urlencoding::decode(val)?;

    if &val[..7] == "file://" {
        println!("{}", &val[7..].replace(" ", "\\ "));
    }
    Ok(())
}

fn prepare_dir_entry(entry: Result<DirEntry, std::io::Error>) -> anyhow::Result<FolderEntry> {
    let entry = entry?;
    if !entry.file_type()?.is_dir() {
        return Err(anyhow!("Didn't expect file type!"));
    }
    let last_modified_at = entry.metadata()?.modified()?;
    let path = entry.path();
    Ok(FolderEntry {
        path,
        last_modified_at,
    })
}

fn collect_items_in_menu_settings(
    mut storage_path: PathBuf,
    with_files: bool,
    with_dirs: bool,
    order: RecentOrder,
) -> anyhow::Result<()> {
    storage_path.push("User/globalStorage/storage.json");
    let file = File::open(storage_path)?;
    let reader = BufReader::new(file);
    let value: sonic_rs::Value = sonic_rs::from_reader(reader)?;
    let items = value
        .as_object_get("lastKnownMenubarData")?
        .as_object_get("menus")?
        .as_object_get("File")?
        .as_object_get("items")?
        .as_array()
        .ok_or_else(|| anyhow!("Failed using field in json as an array!"))?;
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
        .ok_or_else(|| anyhow!("Failed using field in json as an object!"))?
        .iter()
        .filter_map(move |item| {
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
        });

    let uris: Box<dyn Iterator<Item = _>> = match order {
        RecentOrder::Unchanged => Box::new(uris),
        RecentOrder::FilesFirst | RecentOrder::DirsFirst => {
            let (first, second): (Vec<_>, Vec<_>) = uris.partition(|e| {
                // want_file xnor is_file
                !((order == RecentOrder::FilesFirst) ^ (e.t == RecentEntryType::File))
            });
            Box::new(first.into_iter().chain(second))
        }
    };

    for RecentEntry { val, .. } in uris {
        let Ok(val) = urlencoding::decode(val).inspect_err(|err| eprintln!("{err}")) else {
            continue;
        };
        let val = val.replace(" ", "\\ ");
        println!("{val}");
    }
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
    fn as_object_get<'a>(&'a self, key: &str) -> anyhow::Result<&'a sonic_rs::Value>;
}

impl SonicRsValueExtensions for sonic_rs::Value {
    fn as_object_get<'a>(&'a self, key: &str) -> anyhow::Result<&'a sonic_rs::Value> {
        let res = self
            .as_object()
            .ok_or_else(|| anyhow!("Failed using field in json as an object!"))?
            .get(&key)
            .ok_or_else(|| anyhow!("Failed getting field in json!"))?;
        Ok(res)
    }
}
