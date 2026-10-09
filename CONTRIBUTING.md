# Contributing to Saltnitor

First off, thank you for considering contributing to Saltnitor!

Saltnitor is built in Rust using the `ratatui` library. Our goal is to maintain a high-performance, visually dense, and hardware-agnostic terminal dashboard.

## 🧠 Code Architecture (MVC)

The codebase is strictly separated using the Model-View-Controller (MVC) paradigm:

- **`src/app.rs` (The Model)**: Holds the `App` struct. This is the single source of truth for all state (hardware metrics, logs, inputs, etc.).
    
- **`src/events.rs` (The Controller/Events)**: Defines the `Event` enum. All background threads communicate with the main loop by sending these events through a Tokio MPSC channel.
    
- **`src/ui.rs` (The View)**: Contains the `draw` function. It takes the current `App` state and paints the Ratatui widgets onto the terminal.
    
- **`src/main.rs` (The Engine)**: Handles the Tokio async runtime, spawns the hardware/log polling tasks, and manages the main event loop.
    

## 🛠️ Development Setup

1. Fork the repository and clone it locally.
    
2. Ensure you have Rust installed (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
    
3. Run `cargo run` to test locally. Note: GPU features require `nvidia-smi` to be present on your system.
    

## 🚀 Pull Request Process

1. Create a new branch (`git checkout -b feature/your-feature-name`).
    
2. Make your changes. Ensure the UI remains responsive and does not block the main Tokio thread.
    
3. Run `cargo fmt` and `cargo clippy` to ensure code style consistency.
    
4. Submit your PR!
