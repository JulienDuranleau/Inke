# Inke - Screen drawing utility

## Creates an invisible overlay on which to draw.

- Multiple colors
- Brush size control
- Infinite undos and full wipe
- Basic drawing tablet pen pressure
- UI Free (all keyboard shortcuts)
- Quick open/close
- Alt-tab works as with any other apps

## Downloads
[Windows, Mac and linux download links](https://github.com/JulienDuranleau/Inke/releases)

On windows, extract **twice** with a tool like [7-zip](https://www.7-zip.org/) 

No install process


## Shortcuts
| Shortcut    | Action
| :---        | :---
| Escape      | Quit
| Ctrl-z      | Undo
| Spacebar    | Erase everything
| Mouse wheel | Change brush size
| b           | Toggle dark background
| h           | Toggle hidden state (keeps focus)

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

## Compile process
Built with Rust, run `cargo build --release` in this directory.