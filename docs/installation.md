---
icon: lucide/rocket
---

# Installation

## Dependencies

- [ffmpeg](https://www.ffmpeg.org/download.html) (optional, *recommended*) required for transmuxing streams.
- [chrome](https://www.google.com/chrome) / [chromium](https://www.chromium.org/getting-involved/download-chromium/) (optional) needed only for the [capture](https://clitic.github.io/vsd/cli/#vsd-capture) sub-command. 

## Pre-built Binaries

Visit the [releases page](https://github.com/clitic/vsd/releases) for pre-built binaries or grab the [latest CI builds](https://nightly.link/clitic/vsd/workflows/build/main). Extract the binary and add its path to your system's `PATH`.

=== ":fontawesome-brands-windows: Windows"

    Downloads and extracts the `vsd` binary to your current directory.

    === "via Scoop"

        ```bash
        scoop install vsd
        ```
   
    === "x86_64"

        ```powershell
        irm https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-x86_64-pc-windows-msvc.zip -OutFile vsd.zip; Expand-Archive vsd.zip -DestinationPath . -Force; rm vsd.zip
        ```

    === "arm64"
    
        ```powershell
        irm https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-aarch64-pc-windows-msvc.zip -OutFile vsd.zip; Expand-Archive vsd.zip -DestinationPath . -Force; rm vsd.zip
        ```

=== ":fontawesome-brands-linux: Linux"

    Downloads and extracts the `vsd` binary to your current directory.

    === "via Yay (AUR)"

        ```bash
        yay -Syu vsd
        ```

    === "x86_64"

        ```bash
        curl -L https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-x86_64-unknown-linux-musl.tar.xz | tar xJC .
        ```

    === "arm64"
    
        ```bash
        curl -L https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-aarch64-unknown-linux-musl.tar.xz | tar xJC .
        ```

=== ":fontawesome-brands-apple: MacOS"

    Downloads and extracts the `vsd` binary to your current directory.

    === "via Homebrew"

        ```bash
        brew install vsd
        ```

    === "arm64"
    
        ```bash
        curl -L https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-aarch64-apple-darwin.tar.xz | tar xJC .
        ```

=== ":fontawesome-brands-android: Android"

    Downloads and extracts the `vsd` binary to [Termux](https://f-droid.org/en/packages/com.termux)'s `$PREFIX/bin` directory. Also see, [android support](https://clitic.github.io/vsd/android) for more details.

    === "arm64"

        ```bash
        curl -L https://github.com/clitic/vsd/releases/download/vsd-0.5.0/vsd-0.5.0-aarch64-linux-android.tar.xz | tar xJC $PREFIX/bin
        ```

## Install via Cargo

You can also install vsd using cargo.

```bash
cargo install vsd
```
