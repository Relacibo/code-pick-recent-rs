# code-pick-rs
Input for e.g. [rofi](https://github.com/davatorium/rofi) for selecting an entry from a list of recently opened folders/files or a list of workspaces to be opened in vscode. The recent folders/files are extracted from `~/.config/Code/User/globalStorage/storage.json` and the workspaces from `~/.config/Code/User/workspaceStorage`. Please feel free to write an issue, if something doesn't output as expected...

## Install
```bash
cargo install codep
```

or

```bash
cargo install --path .
```

## Usage
I use it as a bind in my `hyperland.conf` with rofi:
```bash
bind = $mainMod SHIFT, C, exec, ~/.cargo/bin/codep recent -a \
    | rofi -dmenu \
    | xargs -r -I {} code --new-window "{}"
bindr = $mainMod&CTRL&SHIFT, C, exec, \
    ~/.cargo/bin/codep -p workspaces -aD -M 365 \
        | rofi -dmenu -markup-rows -display-columns 2 \
        | awk -F '\t' '{print $1}' \
        | xargs -r -I {} code --folder-uri "{}"
```

`codep --help` for more info!

## Environment Variables

`CODEP_CONFIG_ROOT` (default: `~/.config/Code`) - Alternative config root
