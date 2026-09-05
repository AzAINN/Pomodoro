# Releasing pomo

The Cargo package is **`pomo-tui`**; its executable is **`pomo`**. The unrelated
`pomo` package on crates.io is not this project.

## First public release

The source and workflows must be pushed before remote installation commands
will work. This repository uses `main` in the public installer URLs.

1. Review the changes and MIT license, then commit the prepared project.
2. Push the release commit to `main`: `git push origin HEAD:main`.
3. In GitHub repository settings, make `main` the default branch and change the
   repository visibility to Public. Enable Actions if it is disabled.
4. Wait for CI to pass on macOS, Linux, and Windows.
5. Tag the same commit with the version in `Cargo.toml`:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

6. The Release workflow runs CI, verifies the tag against Cargo, builds all
   binaries, and creates a **draft GitHub Release** with checksums.
7. Review that draft and publish it as the latest release. The binary installers
   now work. Publishing is a separate action; creating the tag only creates a draft.

Until a binary release is published, people with Rust can install directly from
the public repository:

```sh
cargo install --git https://github.com/AzAINN/Pomodoro --locked pomo-tui
```

For subsequent versions, update `Cargo.toml`, regenerate `Cargo.lock` with
`cargo check`, update `CHANGELOG.md`, commit, and push a matching `vX.Y.Z` tag.
Never reuse a published version tag. Rerunning the release workflow can replace
assets in a draft; it refuses to overwrite a published release.

## Release assets

| Platform | Archive |
| --- | --- |
| macOS Apple Silicon | `pomo-aarch64-apple-darwin.tar.gz` |
| macOS Intel | `pomo-x86_64-apple-darwin.tar.gz` |
| Linux ARM64 | `pomo-aarch64-unknown-linux-musl.tar.gz` |
| Linux x86-64 | `pomo-x86_64-unknown-linux-musl.tar.gz` |
| Windows x86-64 | `pomo-x86_64-pc-windows-msvc.zip` |

Each archive contains the executable, README, and MIT license. SQLite is bundled
into the executable. Linux uses musl builds. `SHA256SUMS` covers all five archives
and is checked by both installers before replacing a binary.

To inspect packaging locally, using your native target:

```sh
cargo build --release --locked --target aarch64-apple-darwin
python3 scripts/package_release.py --target aarch64-apple-darwin
```

## Optional crates.io publication

GitHub installation and binary releases do not depend on crates.io. To also
support `cargo install pomo-tui --locked`, publish the Cargo package explicitly
using an account authorized to publish `pomo-tui`:

```sh
cargo package --locked
cargo publish --dry-run --locked
cargo login
cargo publish --locked
```

The package name was available when this project was prepared; it is not reserved
until the first successful publish. If it is claimed before then, update the
package name and instructions while retaining the `pomo` binary name.

After publication, verify in a fresh install directory:

```sh
cargo install pomo-tui --locked --root /tmp/pomo-release-check
/tmp/pomo-release-check/bin/pomo --version
```

## Installer options

`POMO_VERSION=v0.2.0` pins a release instead of using the latest. Both installers
accept an absolute `POMO_INSTALL_DIR`.

On macOS/Linux, the default directory is `~/.local/bin`. The installer does not
edit shell configuration. Add that directory to PATH if needed:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

On Windows, the default directory is `%LOCALAPPDATA%\Programs\pomo`. The
PowerShell installer adds it to the current process and user PATH.

Removing the executable uninstalls pomo. User settings and focus history are
kept; `pomo paths` shows their locations before uninstalling.
