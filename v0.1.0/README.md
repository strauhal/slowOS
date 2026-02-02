# SlowOS

```
 ⏳ slowOS  apps                                    12:34
┌─────────────────────────────────────────────────────────┐
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ┌────┐  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │ W  │  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  └────┘  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ slowWrite ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ┌────┐  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │ P  │  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  └────┘  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ slowPaint ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  ┌────┐  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │ B  │  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  └────┘  ░░░│
│░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ slowBooks ░░░│
└─────────────────────────────────────────────────────────┘
 no apps running
```

## Architecture

```
┌─────────────────────────────────────────────┐
│ Buildroot Linux (kernel + busybox)          │
├─────────────────────────────────────────────┤
│ cage (Wayland kiosk compositor)             │
│  or xinit (X11 fallback for e-ink fbdev)    │
├─────────────────────────────────────────────┤
│ slowdesktop (desktop shell)                 │
│  ├── menu bar + clock                       │
│  ├── desktop icons (double-click to launch) │
│  ├── process manager                        │
│  └── about / shutdown dialogs               │
├─────────────────────────────────────────────┤
│ applications (launched as child processes)   │
│  slowWrite    slowPaint    slowBooks         │
│  slowSheets   slowNotes    slowChess         │
│  files        slowMusic    slowSlides        │
│  slowTeX      trash        slowTerm          │
│  slowPics                                    │
├─────────────────────────────────────────────┤
│ slowcore (shared theme, widgets, storage)    │
└─────────────────────────────────────────────┘
```

## Applications

| App | Description |
|-----|-------------|
| slowWrite | word processor (rope-based, vim keys, proportional font) |
| slowPaint | bitmap editor (pencil, shapes, fill) |
| slowBooks | ebook reader (epub, drag-and-drop, CJK fonts) |
| slowSheets | spreadsheet (formulas, multi-cell select) |
| slowNotes | notes (sidebar, search, trash integration) |
| slowChess | chess (full rules, AI opponent) |
| files | file explorer (sort, navigation) |
| slowMusic | music player (rodio, persistent library) |
| slowSlides | presentations (markdown, fullscreen) |
| slowTeX | LaTeX editor (built-in PDF export) |
| trash | trash bin (restore, permanent delete) |
| slowTerm | terminal (shell, history, autocomplete) |
| slowPics | image viewer (memory-efficient large images) |

## Building

Requires Rust 1.70+.

### Development (macOS / Linux)

```bash
# Build everything
cargo build --release --workspace

# Launch the desktop shell
./target/release/slowdesktop

# Or run individual apps
cargo run --release -p slowwrite
cargo run --release -p slowpaint
cargo run --release -p slowbooks
```

### Using the build script

```bash
chmod +x build.sh

./build.sh dev        # debug build
./build.sh release    # optimized build
./build.sh run        # build + launch desktop
./build.sh pi         # cross-compile for Raspberry Pi
./build.sh image      # build complete SD card image
./build.sh clean      # clean artifacts
```

### Raspberry Pi SD Card Image

```bash
./build.sh image
# Flash: dd if=buildroot/.buildroot/output/images/sdcard.img of=/dev/sdX bs=4M
```

## Design

- IBM Plex Sans system font with Noto Sans CJK fallback
- 1px black outlines on everything
- Dithered overlays for selections (classic Mac style)

## License

MIT — by the Slow Computer Company
