# Rust RSS Feed

This project fetches and stores RSS feed items into a SQLite database using Rust and the `reqwest` and `tokio` crates.

## Prerequisites

- Rust and Cargo: Install from [rustup.rs](https://rustup.rs/)
- SQLite: Install SQLite CLI tool or DB Browser for SQLite

## Run & Build

Create a `.env` file in the root directory of the project from `env.template`
```bash
cp .env.template .env
```

### Build
```bash
cargo build
```

### Run
```bash
cargo run
```

## Structure

```bash
./
├── src/
│   └── main.rs
├── .dockerignore
├── .env
├── .env.template
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── Dockerfile
├── Dockerfile.fly
├── data.sqlite
└── fly.toml
```