# slowOS Keyboard Reference

Quick reference for navigating slowOS with a keyboard. On macOS read "Cmd"
as ⌘; on Linux/Windows read it as Ctrl. The OS itself reports it as "Cmd".

## Global (works in every app)

| Key            | Action                                |
|----------------|---------------------------------------|
| Tab            | Cycle focus between UI widgets        |
| Shift+Tab      | Cycle focus backwards                 |
| Enter          | Activate focused button / confirm     |
| Escape         | Close dialog / exit focus mode        |
| Cmd+Plus       | Zoom in                               |
| Cmd+Minus      | Zoom out                              |
| Cmd+0          | Reset zoom (where supported)          |

## Desktop shell (slowdesktop)

| Key            | Action                                |
|----------------|---------------------------------------|
| Cmd+Q          | Shut down dialog                      |
| Cmd+Space      | Open search (Spotlight-style)         |
| Arrow keys     | Navigate selected icon                |
| Enter          | Open selected icon / folder           |

## slowWrite

| Key              | Action                              |
|------------------|-------------------------------------|
| Cmd+N            | New document                        |
| Cmd+O            | Open document                       |
| Cmd+S            | Save                                |
| Cmd+Shift+S      | Save as                             |
| Cmd+B            | Bold toggle                         |
| Cmd+I            | Italic toggle                       |
| Cmd+U            | Underline toggle                    |
| Cmd+1 / 2 / 3    | Heading 1 / 2 / 3                   |
| Cmd+F            | Find                                |
| Cmd+H            | Find and replace                    |
| Cmd+Shift+R      | Toggle plain / rich text view       |
| Cmd+Shift+F      | Focus mode (fullscreen, no chrome)  |

## Files

| Key            | Action                                |
|----------------|---------------------------------------|
| Cmd+Left       | Back                                  |
| Cmd+Right      | Forward                               |
| Cmd+Up         | Parent directory                      |
| Cmd+Delete     | Move to trash (with confirmation)     |
| Cmd+F          | Find                                  |

## Books

| Key            | Action                                |
|----------------|---------------------------------------|
| Left / Right   | Previous / next page                  |
| Cmd+Plus       | Larger text                           |
| Cmd+Minus      | Smaller text                          |
| F              | Toggle fullscreen                     |
| Escape         | Exit fullscreen                       |

## slowView

| Key            | Action                                |
|----------------|---------------------------------------|
| Left / Right   | Previous / next image                 |
| Cmd+Plus       | Zoom in                               |
| Cmd+Minus      | Zoom out                              |
| F              | Toggle fullscreen                     |
| Escape         | Exit fullscreen                       |

## slowMidi

| Key            | Action                                |
|----------------|---------------------------------------|
| Space          | Play / pause                          |
| Cmd+Plus       | Zoom in                               |
| Cmd+Minus      | Zoom out                              |

When an external MIDI device is plugged in (e.g. an OP-1), playback
routes MIDI to that device instead of the internal sine-wave synth.
The status bar shows `-> <device name>` when a device is connected.

## Chess

| Key            | Action                                |
|----------------|---------------------------------------|
| Click piece    | Select                                |
| Click target   | Move                                  |
| Escape         | Deselect                              |

## slowPaint / slowDesign

| Key            | Action                                |
|----------------|---------------------------------------|
| Cmd+Z          | Undo                                  |
| Cmd+Shift+Z    | Redo                                  |
| Cmd+Plus       | Zoom in                               |
| Cmd+Minus      | Zoom out                              |
| V              | Select / move tool                    |
| B              | Brush (slowPaint) / box (slowDesign)  |
| T              | Text (slowDesign)                     |

## USB file transfer

Plug the slowBook into a computer or phone via USB, then in the
slowOS menu → "connect to computer..." → "connect". The slowBook
appears as a drive named `SLOWBOOK` with `Books/`, `Music/`,
`Pictures/`, and `Documents/` folders.

Drop files directly into the matching folder. Files dropped at the
top level of the drive are sorted automatically by extension when
you disconnect.

While connected, slowOS apps can't read user files — eject cleanly
via the "disconnect from computer" menu item before unplugging.
