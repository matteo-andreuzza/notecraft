# NoteCraft

A note-taking app in the style of OneNote, written in Rust. Early alpha: expect missing
features and changes.

## Run

Requires Rust 1.90 or newer.

```sh
cargo run --release -p notecraft                       # start the app
cargo run --release -p notecraft -- --sample           # open the sample notebook
cargo run --release -p notecraft -- ~/Notebooks/Work   # open a notebook folder
cargo run --release -p notecraft -- --help             # all options
```

On Linux it runs on Wayland and X11.

## Develop

```sh
cargo test --workspace   # tests
cargo xtask ci           # all checks
```

## License

MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).

<sub>Microsoft and OneNote are trademarks of Microsoft Corporation. NoteCraft is an independent project, not affiliated with or endorsed by Microsoft.</sub>
