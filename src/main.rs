use serde::Deserialize;
use sonic_rs::{JsonContainerTrait, JsonValueTrait};
use std::{
    fs::{self, File},
    io::BufReader,
    os::unix::process::CommandExt,
    path::PathBuf,
    process,
};

fn main() {
    let storage_path = dirs::config_dir()
        .expect("No config path")
        .join("Code/User/globalStorage/storage.json");
    dbg!(&storage_path);
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
            item.get("id").and_then(|s| s.as_str()) == Some("openRecentFolder")
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
    for (i, uri) in uris.iter().enumerate() {
        println!("{i}: {uri}");
    }
    let input: String = text_io::read!();
    let num: usize = input.parse().expect("Need index!");
    if num >= uris.len() {
        panic!("Index too big!");
    }

    let path = uris[num];

    println!("Opening {path} in VSCode!");

    process::Command::new("code")
        .arg(path)
        .output()
        .unwrap();
}
