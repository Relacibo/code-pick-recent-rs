use clap::Parser;
use sonic_rs::{JsonContainerTrait, JsonValueTrait};
use std::{fs::File, io::BufReader, path::PathBuf};

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
}
fn get_default_config_root() -> PathBuf {
    dirs::config_dir().expect("No config path").join("Code")
}

fn main() {
    let Args {
        config_root,
        no_files,
        no_dirs,
    } = Args::parse();
    let config_root = config_root.unwrap_or_else(get_default_config_root);
    let storage_path = config_root.join("User/globalStorage/storage.json");
    let file = File::open(storage_path).unwrap();
    let reader = BufReader::new(file);

    let value: sonic_rs::Value = sonic_rs::from_reader(reader).unwrap();
    let items = value
        .as_object()
        .unwrap()
        .get(&"lastKnownMenubarData")
        .unwrap()
        .as_object()
        .unwrap()
        .get(&"menus")
        .unwrap()
        .as_object()
        .unwrap()
        .get(&"File")
        .unwrap()
        .as_object()
        .unwrap()
        .get(&"items")
        .unwrap()
        .as_array()
        .unwrap();
    let recent = items
        .iter()
        .find(|item| {
            item.get("id").and_then(|s| s.as_str()) == Some("submenuitem.MenubarRecentMenu")
        })
        .unwrap();
    let uris = recent
        .as_object()
        .unwrap()
        .get(&"submenu")
        .unwrap()
        .as_object()
        .unwrap()
        .get(&"items")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .filter(|item| {
            let id = item.get("id").and_then(|s| s.as_str());
            (!no_files && id == Some("openRecentFolder")
                || !no_dirs && id == Some("openRecentFile"))
                && item.get("enabled").and_then(|s| s.as_bool()) == Some(true)
        })
        .map(|item| {
            item.as_object()
                .unwrap()
                .get(&"uri")
                .unwrap()
                .as_object()
                .unwrap()
                .get(&"path")
                .unwrap()
                .as_str()
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut out = uris.join("\n");
    out.push('\0');
    println!("{out}");
}
