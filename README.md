<p align="center">
  <img src="data/icons/scalable/apps/io.github.danst0.wifichecker.png" width="128" alt="WiFi Checker icon"/>
</p>

<h1 align="center">WiFi Checker</h1>

<p align="center">
  A GTK4 desktop app for mapping WiFi signal strength across building floors.
  Draw floor plans, take measurements, and visualize coverage as a live heatmap.
</p>

<p align="center">
  <a href="https://github.com/danst0/wifichecker/actions/workflows/ci.yml">
    <img src="https://github.com/danst0/wifichecker/actions/workflows/ci.yml/badge.svg" alt="CI/CD"/>
  </a>
  <img src="https://img.shields.io/badge/platform-Linux-blue" alt="Platform: Linux"/>
  <img src="https://img.shields.io/badge/built%20with-Rust%20%2B%20GTK4-orange" alt="Rust + GTK4"/>
</p>

---

## Features

- **Interactive floor plans** — import a background image or draw your own layout freehand
- **Multi-floor support** — manage multiple floors within a single project
- **One-click measurements** — click anywhere on the map to capture WiFi signal data at that point
- **Live heatmap overlay** — signal strength is visualised as a colour-coded grid (red → green)
- **Throughput testing** — optional iperf3/iperf2 and Samba speed tests run alongside each WiFi scan
- **Calibration** — set a real-world scale by clicking two known points and entering the distance in metres
- **Snap-to-grid** — configurable measurement grid with optional snap for consistent coverage
- **Zoom & pan** — scroll-wheel zoom (cursor-centred) plus zoom in/out/reset buttons
- **Auto-save** — project, drawings, and settings persist automatically to `~/.config/wifichecker/`

---

## Screenshots

> _Add screenshots here_

---

## Installation

### Flatpak (recommended)

Download the latest `.flatpak` bundle from the [Releases](https://github.com/danst0/wifichecker/releases) page and install it:

```bash
flatpak install wifichecker.flatpak
flatpak run io.github.danst0.wifichecker
```

### Build from source

**Prerequisites**

| Dependency | Purpose |
|---|---|
| Rust (stable) | Build toolchain |
| GTK 4.12+ | GUI framework |
| libadwaita 1.4+ | GNOME adaptive UI |
| `nmcli` | WiFi scanning (NetworkManager) |
| `iperf3` or `iperf2` | Throughput testing _(optional)_ |
| `smbclient` | Samba share testing _(optional)_ |

On Fedora/RHEL:
```bash
sudo dnf install gtk4-devel libadwaita-devel NetworkManager
```

On Debian/Ubuntu:
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev network-manager
```

**Build & run**

```bash
git clone https://github.com/danst0/wifichecker.git
cd wifichecker
cargo build --release
./target/release/wifichecker
```

---

## Usage

### Taking measurements

1. Select or add a floor from the dropdown at the top
2. _(Optional)_ Import a floor plan image via the **Import** button
3. Make sure you are in **Measure** mode (default)
4. Click anywhere on the map — WiFi data is captured immediately and a coloured cell appears
5. Repeat across the area you want to survey

### Calibrating the scale

1. Switch to **Calibrate** mode
2. Click two points on the map that correspond to a known real-world distance
3. Enter the distance in metres when prompted
4. The grid spacing will now reflect accurate metre values

### Drawing a floor plan

1. Switch to **Draw** mode
2. Click and drag to draw orthogonal lines (automatically snapped to the grid)
3. Adjust stroke width with the spinner in the toolbar

### Throughput testing

Open **Settings** (the network icon in the header) and enable:

- **iperf3 speed test** — enter your iperf3 server address, port, and test duration
- **Samba speed test** — enter your SMB server, share name, and credentials

Both tests run in the background alongside every WiFi scan and their results are stored with each measurement.

---

## Settings

| Setting | Description |
|---|---|
| iperf3 server / port / duration | Configure the throughput test endpoint |
| Samba server / share / credentials | Configure the SMB share test |
| Grid spacing | Visual grid density on the map |
| Measurement cell spacing | Granularity of the heatmap grid |
| Snap to grid | Align measurement points to grid centres |
| Throughput units | Display speeds as Mbit/s or MB/s |

---

## Data storage

All data is stored under `~/.config/wifichecker/`:

```
~/.config/wifichecker/
├── project.json        # floors, measurements, calibration
├── settings.json       # app preferences
└── drawings/
    ├── floor_0.png     # drawn floor plan for floor 0
    ├── floor_1.png
    └── ...
```

---

## Contributing

Contributions are welcome. Please open an issue first for anything beyond a small bug fix, so we can discuss the approach.

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Lint
cargo clippy
```

---

## License

This project is open source. See [LICENSE](LICENSE) for details.
