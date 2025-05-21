# code-pick-rs
Input for e.g. wofi for selecting an entry from a list of recently opened folders/files or a list of workspaces to be opened in vscode. The recent folders/files are extracted from `~/.config/Code/User/globalStorage/storage.json` and the workspaces from `~/.config/Code/User/workspaceStorage`.

## Install
```bash
cargo install --path .
```

## Usage
I use it as a bind in my `hyperland.conf` with wofi:
```bash
bind = $mainMod SHIFT, C, exec, ~/.cargo/bin/codep recent -a | wofi --dmenu | xargs -r -I {} code --new-window "{}"
bindr = $mainMod&CTRL&SHIFT, C, exec, ~/.cargo/bin/codep workspaces -x 365 | wofi --dmenu | xargs -r -I {} code --new-window "{}"
```

`codep --help` for more info!

## Environment Variables

`CODEP_CONFIG_ROOT` (default: `~/.config/Code`) - Alternative config root
