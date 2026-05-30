<h1 align="center">vsd</h1>

[![Github Downloads](https://img.shields.io/github/downloads/clitic/vsd/total?logo=github&style=flat-square)](https://github.com/clitic/vsd/releases)
[![Crate Downloads](https://img.shields.io/crates/d/vsd?logo=rust&style=flat-square)](https://crates.io/crates/vsd)
[![Crate Version](https://img.shields.io/crates/v/vsd?style=flat-square)](https://crates.io/crates/vsd)
[![Build Status](https://img.shields.io/github/actions/workflow/status/clitic/vsd/build.yml?logo=github&style=flat-square)](https://github.com/clitic/vsd/actions)
[![Crate License](https://img.shields.io/crates/l/vsd?style=flat-square)](https://crates.io/crates/vsd)
[![Repo Size](https://img.shields.io/github/repo-size/clitic/vsd?logo=github&style=flat-square)](https://github.com/clitic/vsd)
[![Open In Colab](https://img.shields.io/badge/Open%20In%20Colab-F9AB00?logo=googlecolab&color=525252&style=flat-square)](https://colab.research.google.com/github/clitic/vsd/blob/main/vsd/vsd-on-colab.ipynb)

**V**ideo **S**tream **D**ownloader is a powerful command-line utility that enables users to download video content streamed over HTTP from websites. It supports both [DASH (Dynamic Adaptive Streaming over HTTP)](https://en.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP) using `.mpd` manifest files and [HLS (HTTP Live Streaming)](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) using `.m3u8` playlists. The tool is designed to handle adaptive bitrate streams, fetch individual video and audio segments, and optionally mux them into a single playable file, making it ideal for offline viewing, archival, or analysis of online video content.

<div align="center">
  <img src="https://raw.githubusercontent.com/clitic/vsd/refs/heads/main/docs/images/showcase.gif" width="700px">
</div>

## Features

- [x] Captures network requests and lists playlist and subtitle files from websites.
- [x] Compatible with both DASH and HLS playlists.
- [x] Enables multi-threaded downloading for faster performance.
- [x] Muxing streams to single video container using ffmpeg.
- [x] Offers robust automation support.
- [x] One unified progress bar tracking the entire download, with real-time file size updates.
- [x] Supports decryption for `AES-128`, `SAMPLE-AES`, `CENC`, `CENS`, `CBC1` and `CBCS`.
- [ ] Live stream downloading, consider [contributing](https://github.com/clitic/vsd/fork) this feature.

## [Installation](https://clitic.github.io/vsd/install)
  
### Dependencies

- [ffmpeg](https://www.ffmpeg.org/download.html) (optional, *recommended*) required for transmuxing and transcoding streams.
- [chrome](https://www.google.com/chrome) / [chromium](https://www.chromium.org/getting-involved/download-chromium/) (optional) needed only for the capture sub-command. 

### Pre-built Binaries

Visit the [releases page](https://github.com/clitic/vsd/releases) for pre-built binaries or grab the [latest CI builds](https://nightly.link/clitic/vsd/workflows/build/main).
Download and extract the archive, then copy the vsd binary to a directory of your choice.
Finally, add that directory to your system's `PATH` environment variable.

[![Packaging Status](https://repology.org/badge/vertical-allrepos/vsd.svg)](https://repology.org/project/vsd/versions)

### Install via Cargo

You can also install vsd using cargo.

```bash
cargo install vsd
```

### Additional Resources

- [Build Instructions](https://clitic.github.io/vsd/build)
- [Changelog](https://clitic.github.io/vsd/CHANGELOG)

## [Usage](https://clitic.github.io/vsd/usage)

Below are detailed usage scenarios for `vsd`. For complete option details, see the [cli reference](https://clitic.github.io/vsd/cli).

### Network Capturing

Capture network requests, playlist file links (`.m3u8`, `.mpd`), and subtitle paths directly from an automated browser instance.

* **Basic request capturing:**
  ```bash
  vsd capture <url> --save-cookies
  ```
  > The saved cookies will be written to `cookies.txt` and can be used as `--cookies cookies.txt` in subsequent download sub-commands.

* **Headless request capturing (without GUI window):**
  ```bash
  vsd capture <url> --headless --save-cookies
  ```

---

### Downloading & Format Selection (`save`)

Download DASH and HLS video streams with powerful format selection inspired by yt-dlp.

* **Basic playlist download (best video + best audio + subtitle):**
  ```bash
  vsd save <url> -o video.mp4
  ```
  > Add `-i, --interactive` to open a styled stream selection menu.

* **List available streams:**
  ```bash
  vsd save <url> -F
  ```

* **Select streams by index (from `-F` output):**
  ```bash
  vsd save <url> -f "1+3" -o video.mp4
  ```

* **Select 720p or lower video with best audio:**
  ```bash
  vsd save <url> -f "bv[height<=720]+ba" -o video.mp4
  ```

* **Select English audio, skip subtitles:**
  ```bash
  vsd save <url> -f "bv+ba[lang=en]" -o video.mp4
  ```

* **Clip specific timeline sections (accurate to segments):**
  ```bash
  vsd save <url> --clip 01:00-01:30 -o clip.mp4
  ```

* **Tune concurrent download threads (speed up downloads):**
  ```bash
  vsd save <url> --threads 8 -o video.mp4
  ```

---

### Decryption & DRM (`save`, `decrypt`, `license`)

Decrypt protected content by supplying keys directly or querying CDM servers.

* **Download DRM stream with known keys:**
  ```bash
  vsd save "https://media.axprod.net/TestVectors/Dash/protected_dash_1080p_h264_singlekey/manifest.mpd" \
      --keys "4060a865887842679cbf91ae5bae1e72:fc35340837310cc0fb53de97e22a69e0" \
      -o video.mp4
  ```

* **Decrypt a local fragmented CENC MP4 file:**
  ```bash
  vsd decrypt --key "fc35340837310cc0fb53de97e22a69e0" encrypted-video.mp4 -o decrypted.mp4
  ```

* **Decrypt with a separate initialisation segment:**
  ```bash
  vsd decrypt --key "fc35340837310cc0fb53de97e22a69e0" --init init.mp4 encrypted-video.mp4 -o decrypted.mp4
  ```

* **Request keys from Widevine license servers using a CDM device file:**
  ```bash
  vsd license --widevine-url "https://cwip-shaka-proxy.appspot.com/no_auth" --widevine-device device.wvd video-init.mp4
  ```

---

### Post-Processing (`extract`, `merge`)

Manipulate media streams and extract metadata easily.

* **Extract subtitles (`wvtt` or `stpp`) from fragmented MP4:**
  ```bash
  vsd extract media-subs.mp4 -o subtitles.srt
  ```

* **Merge multiple fragmented media segments:**
  ```bash
  vsd merge segment_*.m4s --type binary -o merged.mp4
  ```

* **Parse playlist to structured JSON metadata:**
  ```bash
  vsd save <url> --parse > parsed-playlist.json
  ```
  > View the [JSON schema](https://github.com/clitic/vsd/blob/main/vsd/src/playlist/types.rs) of the output metadata.

## Donate

This project is developed and maintained in my free time. Donations help cover development time, testing, and future improvements. If this tool saved you time or helped your workflow, consider supporting it.

<div align="center">
  <a href="mailto:clitic21@gmail.com" target="_blank" style="text-decoration: none;">
    <img src="https://raw.githubusercontent.com/clitic/vsd/refs/heads/main/docs/assets/contact.svg" alt="Contact Me" height="42px">
  </a>
  <a href="https://www.buymeacoffee.com/clitic" target="_blank" style="text-decoration: none;">
    <img src="https://raw.githubusercontent.com/clitic/vsd/refs/heads/main/docs/assets/bmc.svg" alt="Buy Me A Coffee" height="40px">
  </a>
  <a href="https://paypal.me/clitic" target="_blank" style="text-decoration: none;">
    <img src="https://raw.githubusercontent.com/clitic/vsd/refs/heads/main/docs/assets/paypal.svg" alt="PayPal" height="40px">
  </a>
</div>

## License

Dual Licensed

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0) ([LICENSE-APACHE](LICENSE-APACHE))
- [MIT license](https://opensource.org/licenses/MIT) ([LICENSE-MIT](LICENSE-MIT))
