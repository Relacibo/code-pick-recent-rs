use anyhow::anyhow;
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use sonic_rs::{JsonContainerTrait, JsonValueTrait};
use std::{
    fmt::{Debug, Display},
    fs::{self, DirEntry, File},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[arg(short, long)]
    config_root: Option<PathBuf>,

    #[arg[short = '0', long]]
    null_terminated: bool,

    #[arg[short = 'p', long]]
    use_pango_markup: bool,

    #[arg[short, long]]
    all: bool,

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
        #[arg(short = 'W', long)]
        with_dirs: bool,
        #[arg(short = 'r', long)]
        with_remotes: bool,
        #[arg(short, long)]
        all: bool,

        #[arg(short = 'D', long)]
        create_display_strings: bool,

        #[arg(short = 'M', long)]
        max_age_days: Option<u32>,

        #[arg(short, long)]
        limit: Option<usize>,
    },
    History {
        #[arg(short = 'W', long)]
        with_dirs: bool,
        #[arg(short = 'r', long)]
        with_remotes: bool,
        #[arg(short, long)]
        all: bool,

        #[arg(short = 'D', long)]
        create_display_strings: bool,

        #[arg(short = 'M', long)]
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
        all: global_all,
        null_terminated,
        use_pango_markup,
        command,
    } = Args::parse();
    let config_root = config_root
        .or_else(|| std::env::var("CODEP_CONFIG_ROOT").ok().map(PathBuf::from))
        .unwrap_or_else(get_default_config_root);

    match command {
        Command::Recent {
            with_files,
            with_dirs,
            all,
            order,
        } => {
            let all = global_all || all;
            collect_items_in_menu_settings(
                config_root,
                null_terminated,
                all || with_files,
                all || with_dirs,
                order,
            )?;
        }
        Command::Workspaces {
            with_dirs,
            with_remotes,
            all,
            create_display_strings,
            max_age_days,
            limit,
        } => {
            let all = global_all || all;
            collect_items_in_workspaces(
                config_root,
                max_age_days,
                limit,
                all || with_dirs,
                all || with_remotes,
                null_terminated,
                create_display_strings,
                use_pango_markup,
            )?;
        }
        Command::History {
            with_dirs,
            with_remotes,
            all,
            create_display_strings,
            max_age_days,
            limit,
        } => {
            let all = global_all || all;
            collect_items_in_history(
                config_root,
                limit,
                max_age_days,
                all || with_dirs,
                all || with_remotes,
                null_terminated,
                create_display_strings,
                use_pango_markup,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_items_in_workspaces(
    mut storage_path: PathBuf,
    max_age_days: Option<u32>,
    limit: Option<usize>,
    with_dirs: bool,
    with_remotes: bool,
    null_terminated: bool,
    create_display_strings: bool,
    use_pango_markup: bool,
) -> anyhow::Result<()> {
    storage_path.push("User/workspaceStorage");

    let min_system_time = max_age_days
        .map(get_min_system_time_from_max_age_days)
        .transpose()?;

    let mut entries = fs::read_dir(&storage_path)?
        .filter_map(|entry| match get_data_from_dir_entry(entry) {
            Err(err) => {
                eprintln!("Error at: {}", &storage_path.as_os_str().to_string_lossy());
                eprintln!("Error reading workspace entry! {err}");
                None
            }
            Ok(entry) => {
                if let Some(min_system_time) = min_system_time {
                    if entry.last_modified_at < min_system_time {
                        return None;
                    }
                }
                Some(entry)
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|e1, e2| e1.last_modified_at.cmp(&e2.last_modified_at).reverse());

    let limit = limit.unwrap_or(usize::MAX);

    for FolderEntry { path, .. } in entries.into_iter().take(limit) {
        let path = path.join("workspace.json");
        if let Err(err) = digest_workspaces_dir_entry(
            &path,
            with_dirs,
            with_remotes,
            null_terminated,
            create_display_strings,
            use_pango_markup,
        ) {
            eprintln!("Error with file: {}", &path.as_os_str().to_string_lossy());
            eprintln!("Error digesting workspace entry! {err}");
        }
    }
    Ok(())
}

fn get_min_system_time_from_max_age_days(max_age_days: u32) -> anyhow::Result<SystemTime> {
    const NUM_SECONDS_IN_DAY: u64 = 86400;
    let res = SystemTime::now()
        .checked_sub(Duration::from_secs(
            (max_age_days as u64) * NUM_SECONDS_IN_DAY,
        ))
        .ok_or_else(|| anyhow!("`max-age-days` too big"))?;
    Ok(res)
}

#[derive(Clone, Debug)]
struct FolderEntry {
    path: PathBuf,
    last_modified_at: SystemTime,
}

fn digest_history_dir_entry(
    path: &Path,
    null_terminated: bool,
    with_dirs: bool,
    with_remotes: bool,
    create_display_strings: bool,
    use_pango_markup: bool,
) -> anyhow::Result<()> {
    if !fs::exists(path)? {
        return Ok(());
    }
    let mut file = File::open(path)?;
    let mut v: Vec<u8> = Vec::new();
    file.read_to_end(&mut v)?;
    let value: sonic_rs::Value = sonic_rs::from_slice(&v)?;

    let Ok(field) = value.as_object_get_result("resource") else {
        return Ok(());
    };
    let val = field.as_str_result()?;
    digest_folder_uri(
        val,
        null_terminated,
        use_pango_markup,
        with_dirs,
        with_remotes,
        create_display_strings,
    )?;
    Ok(())
}

fn digest_workspaces_dir_entry(
    path: &Path,
    with_dirs: bool,
    with_remotes: bool,
    null_terminated: bool,
    create_display_strings: bool,
    use_pango_markup: bool,
) -> anyhow::Result<()> {
    if !fs::exists(path)? {
        return Ok(());
    }
    let mut file = File::open(path)?;
    let mut v: Vec<u8> = Vec::new();
    file.read_to_end(&mut v)?;
    let value: sonic_rs::Value = sonic_rs::from_slice(&v)?;

    let Ok(field) = value.as_object_get_result("folder") else {
        return Ok(());
    };
    let val = field.as_str_result()?;
    digest_folder_uri(
        val,
        null_terminated,
        use_pango_markup,
        with_dirs,
        with_remotes,
        create_display_strings,
    )?;
    Ok(())
}

fn digest_folder_uri(
    val: &str,
    null_terminated: bool,
    use_pango_markup: bool,
    with_dirs: bool,
    with_remotes: bool,
    create_display_strings: bool,
) -> anyhow::Result<()> {
    let val = urlencoding::decode(val)?;

    let starts_with_file = with_dirs && val.starts_with("file://");
    let starts_with_remote = with_remotes && val.starts_with("vscode-remote://");

    if !starts_with_file && !starts_with_remote {
        return Ok(());
    }

    let clean_val = val.replace("\t", "").replace("\n", "").replace("\0", "");

    if create_display_strings {
        print!("{clean_val}\t");
        if starts_with_file {
            print!("{}", &val[7..]);
        } else if starts_with_remote {
            match extract_folder_name_from_remote_val(&val[16..]) {
                Err(err) => {
                    eprintln!("Couldn't parse `vscode-remote` folder-string! ");
                    eprintln!("{err}");
                    print!("{clean_val}");
                }
                Ok(r) => {
                    print_display_info(&r, use_pango_markup);
                }
            }
        } else {
            print!("{clean_val}");
        }
    } else {
        print!("{clean_val}");
    };

    if null_terminated {
        print!("\0");
    }
    println!();
    Ok(())
}

fn extract_folder_name_from_remote_val(rest: &str) -> anyhow::Result<DisplayInfo> {
    let remote_type_end = rest
        .chars()
        .position(|c| c == '+')
        .ok_or_else(|| anyhow!("No space found!"))?;
    let hex_start = remote_type_end + 1;
    let hex_end = rest[hex_start..]
        .chars()
        .position(|c| c == '/')
        .ok_or_else(|| anyhow!("No slash found after first space!"))?
        + hex_start;

    let remote_type = &rest[..remote_type_end];
    let remote_type = get_display_string_from_remote_type(remote_type);

    // Hex decode
    let Ok(v) = (hex_start..hex_end)
        .step_by(2)
        .map(|i| u8::from_str_radix(&rest[i..i + 2], 16).map(|u| u as char))
        .collect::<Result<String, _>>()
    else {
        return Ok(DisplayInfo {
            val: rest[hex_start..].to_owned(),
            hint: Some(DisplayInfoHint {
                remote_type: remote_type.to_string(),
                addition: None,
            }),
        });
    };

    let info = if let Some((val, addition)) = hint_addition_from_json_slice(&v) {
        DisplayInfo {
            val,
            hint: Some(DisplayInfoHint {
                remote_type: remote_type.to_string(),
                addition,
            }),
        }
    } else {
        DisplayInfo {
            val: v,
            hint: Some(DisplayInfoHint {
                remote_type: remote_type.to_string(),
                addition: None,
            }),
        }
    };

    Ok(info)
}

fn get_display_string_from_remote_type(remote_type: &str) -> &str {
    match remote_type {
        "dev-container" => "Dev Container",
        "ssh-remote" => "SSH Remote",
        v => v,
    }
}

fn hint_addition_from_json_slice(v: &str) -> Option<(String, Option<&'static str>)> {
    let val: sonic_rs::Value = sonic_rs::from_str(v).ok()?;
    let obj = val.as_object()?;
    for path in ["hostPath", "repositoryPath", "volumeName"] {
        let Some(s) = obj.get(&path) else {
            continue;
        };
        let Some(s) = s.as_str() else {
            continue;
        };
        return Some((s.to_owned(), hint_addition_from_path(path)));
    }
    None
}

fn hint_addition_from_path(path: &str) -> Option<&'static str> {
    match path {
        "hostPath" => None,
        "repositoryPath" => Some("repository"),
        "volumeName" => Some("volume"),
        _ => Some("unknown"),
    }
}

fn get_data_from_dir_entry(entry: Result<DirEntry, std::io::Error>) -> anyhow::Result<FolderEntry> {
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
    null_terminated: bool,
    with_files: bool,
    with_dirs: bool,
    order: RecentOrder,
) -> anyhow::Result<()> {
    storage_path.push("User/globalStorage/storage.json");
    let file = File::open(storage_path)?;
    let reader = BufReader::new(file);
    let value: sonic_rs::Value = sonic_rs::from_reader(reader)?;
    let items = value
        .as_object_get_result("lastKnownMenubarData")?
        .as_object_get_result("menus")?
        .as_object_get_result("File")?
        .as_object_get_result("items")?
        .as_array()
        .ok_or_else(|| anyhow!("Failed using field in json as an array!"))?;
    let recent = items
        .iter()
        .find(|item| {
            let Ok(id) = item.as_object_get_result("id") else {
                return false;
            };
            let Some(id) = id.as_str() else {
                return false;
            };
            id == "submenuitem.MenubarRecentMenu"
        })
        .ok_or_else(|| anyhow!("Didn't find menubar!"))?;
    let uris = recent
        .as_object_get_result("submenu")?
        .as_object_get_result("items")?
        .as_array()
        .ok_or_else(|| anyhow!("Failed using field in json as an object!"))?
        .iter()
        .filter_map(move |item| {
            let id = item.as_object_get_result("id").ok()?.as_str()?;
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
                .as_object_get_result("uri")
                .ok()?
                .as_object_get_result("path")
                .ok()?
                .as_str()?;
            let t = match id {
                "openRecentFile" => RecentEntryType::File,
                "openRecentFolder" => RecentEntryType::Dir,
                _ => {
                    eprintln!("Unsupported entry type id!");
                    return None;
                }
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
        print!(
            "{}",
            val.trim()
                .replace("\t", "")
                .replace("\n", "")
                .replace("\0", "")
        );
        if null_terminated {
            print!("\0");
        }
        println!();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_items_in_history(
    mut storage_path: PathBuf,
    limit: Option<usize>,
    max_age_days: Option<u32>,
    with_dirs: bool,
    with_remotes: bool,
    null_terminated: bool,
    create_display_strings: bool,
    use_pango_markup: bool,
) -> anyhow::Result<()> {
    storage_path.push("User/History");

    let min_system_time = max_age_days
        .map(get_min_system_time_from_max_age_days)
        .transpose()?;

    let mut entries = fs::read_dir(&storage_path)?
        .filter_map(|entry| match get_data_from_dir_entry(entry) {
            Err(err) => {
                eprintln!("Error at: {}", &storage_path.as_os_str().to_string_lossy());
                eprintln!("Error reading history entry! {err}");
                None
            }
            Ok(entry) => {
                if let Some(min_system_time) = min_system_time {
                    if entry.last_modified_at < min_system_time {
                        return None;
                    }
                }
                Some(entry)
            }
        })
        .collect::<Vec<_>>();

    entries.sort_by(|e1, e2| e1.last_modified_at.cmp(&e2.last_modified_at).reverse());

    let limit = limit.unwrap_or(usize::MAX);

    for FolderEntry { path, .. } in entries.into_iter().take(limit) {
        let path = path.join("entries.json");
        if let Err(err) = digest_history_dir_entry(
            &path,
            null_terminated,
            with_dirs,
            with_remotes,
            create_display_strings,
            use_pango_markup,
        ) {
            eprintln!("Error with file: {}", &path.as_os_str().to_string_lossy());
            eprintln!("Error digesting workspace entry! {err}");
        }
    }
    Ok(())
}

trait SonicRsValueExtensions {
    type ObjectType;
    fn as_object_get_result<'a>(&'a self, key: &str) -> anyhow::Result<&'a sonic_rs::Value>;
    fn as_str_result(&self) -> anyhow::Result<&str>;
}

impl SonicRsValueExtensions for sonic_rs::Value {
    type ObjectType = sonic_rs::Object;
    fn as_object_get_result<'a>(&'a self, key: &str) -> anyhow::Result<&'a sonic_rs::Value> {
        let res = self
            .as_object()
            .ok_or_else(|| anyhow!("Failed using field in json as an object!"))?
            .get(&key)
            .ok_or_else(|| anyhow!("Failed getting field in json!"))?;
        Ok(res)
    }

    fn as_str_result(&self) -> anyhow::Result<&str> {
        let res = self
            .as_str()
            .ok_or_else(|| anyhow!("Failed using field in json as a string!"))?;
        Ok(res)
    }
}

#[derive(Clone, Debug)]
struct DisplayInfo {
    val: String,
    hint: Option<DisplayInfoHint>,
}

fn print_display_info(val: &DisplayInfo, use_pango_markup: bool) {
    let DisplayInfo { val, hint } = val;
    print!("{val}");
    if let Some(hint) = hint {
        let DisplayInfoHint {
            remote_type,
            addition,
        } = hint;
        print!(" ");
        if use_pango_markup {
            print!("<small>");
        }
        print!("({remote_type}");
        if let Some(addition) = addition {
            print!("|{addition}");
        }
        if use_pango_markup {
            print!("</small>");
        }
        print!(")");
    }
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

#[derive(Clone, Debug)]
struct DisplayInfoHint {
    remote_type: String,
    addition: Option<&'static str>,
}

impl Display for DisplayInfoHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let DisplayInfoHint {
            remote_type,
            addition,
        } = self;
        f.write_fmt(format_args!(" ({remote_type}"))?;
        if let Some(addition) = addition {
            f.write_fmt(format_args!("|{addition}"))?;
        }
        f.write_fmt(format_args!(")"))?;
        Ok(())
    }
}
