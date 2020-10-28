# Inke - Screen drawing utility

## Creates a transparent overlay over your screen(s) on which to draw.

- Multiple colors brush
- Brush size control
- Infinite undos and instant wipe
- Basic drawing tablet pen pressure
- Clutter free (no UI, all keyboard shortcuts based)
- Quick open/close
- Alt-tab works as with any other apps

## Downloads
[Windows, Mac and linux download links](https://github.com/JulienDuranleau/Inke/releases)

## Install on Windows/Linux
No install process, download, unzip in a folder, launch

## Install on MacOSX
Download, unzip in a folder and move the app to your `/Applications` folder. Authorize app in your security panel if OSX doesn't let you launch it the first time.

## Shortcuts
| Shortcut    | Action
| :---        | :---
| Escape      | Quit
| Ctrl-z      | Undo (Windows, Linux)
| Cmd-z       | Undo (Mac)
| Spacebar    | Erase everything
| Mouse wheel | Change brush size
| b           | Toggle background

For a good workflow, I strongly suggest using a shortcut such as Windows-1 to launch it from your taskbar and escape out of it with the `escape` key when you're done.

---

| Color Shortcut | Color
| :---           | :---
| q              | White
| w              | Black
| e              | Orange
| r              | Purple
| t              | Red
| y              | Green
| u              | Blue
| i              | Yellow

---

| Brush Size Shortcut | Size Preset
| :---        | :---
| 1           | Smallest brush
| 2           | Smaller brush
| 3           | Regular brush
| 4           | Big brush
| 5           | Huge brush

## Configurations
Colors, brush sizes, smoothing and background color and opacity are stored in `config.json` next to the executable file after the first launch.

## Compile process
1. Install Rust with [https://rustup.rs/](https://rustup.rs/)
2. Clone repo
3. run `cargo run` in the root directory
