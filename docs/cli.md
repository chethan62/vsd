---
icon: lucide/terminal
---

# VSD CLI

This document contains cli reference for the `vsd` command-line program.

## Command Overview

- [`vsd`↴](#vsd)
- [`vsd capture`↴](#vsd-capture)
- [`vsd decrypt`↴](#vsd-decrypt)
- [`vsd extract`↴](#vsd-extract)
- [`vsd license`↴](#vsd-license)
- [`vsd merge`↴](#vsd-merge)
- [`vsd save`↴](#vsd-save)

## `vsd`

A command-line utility and library for downloading DASH and HLS playlists.

```
vsd [OPTIONS] <COMMAND>
```

**Subcommands:**

| Command | Description |
|---------|-------------|
| `capture` | Capture playlist requests from a website |
| `decrypt` | Decrypt CENC encrypted mp4 files |
| `extract` | Extract subtitles from a fragmented mp4 file |
| `license` | Request content keys from a license server |
| `merge` | Merge multiple media segments into a single file |
| `save` | Download streams from DASH or HLS playlist |

**Global Options:**

| Flag | Description |
|------|-------------|
| `--color` | Enable colored output<br>*Possible values:* `auto`, `always`, `never`<br>*Default:* `auto` |
| `-q, --quiet` | Disable all output except errors |
| `-v, --verbose` | Enable more detailed logging. Use -v (debug) or -vv (trace) |

[↑ Back to top](#command-overview)

### `vsd capture`

Capture playlist requests from a website.

Requires any one of these browsers:

- [chrome](https://www.google.com/chrome)
- [chromium](https://www.chromium.org/getting-involved/download-chromium)

This command launches an automated browser instance and listen on network requests. Behavior may vary and it may not work as expected on all websites. This is equivalent to manually doing:

Inspect -> Network -> Fetch/XHR -> Filter by extension -> Copy as cURL (bash)

```
vsd capture [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: https://.. *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `--cookies` | Launch browser with cookies (netscape cookie file) |
| `--extensions` | List of file extensions to be filtered out separated by comma<br>*Default:* `.m3u,.m3u8,.mpd,.vtt,.ttml,.srt` |
| `--headless` | Launch browser in headless mode (without a window) |
| `--proxy` | Launch browser with a proxy |
| `--resource-types` | List of resource types to be filtered out separated by comma<br>*Possible values:* `document`, `stylesheet`, `image`, `media`, `font`, `script`, `texttrack`, `xhr`, `fetch`, `prefetch`, `eventsource`, `websocket`, `manifest`, `signedexchange`, `ping`, `cspviolationreport`, `preflight`, `fedcm`, `other`<br>*Default:* `fetch,xhr` |
| `--save-cookies` | Save browser cookies in cookies.txt (netscape cookie file) |

[↑ Back to top](#command-overview)

### `vsd decrypt`

Decrypt CENC encrypted mp4 files.

Supports cenc, cens, cbc1 and cbcs protection schemes.

```
vsd decrypt [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: Fragmented mp4 file path to decrypt *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `-k, --key` | Decryption key in hex format (32 hex characters) |
| `--init` | Path to a separate init segment containing moov/tenc boxes |
| `-o, --output` | Output file path for the decrypted file.<br><br>Defaults to the input filename with a .dec suffix before the extension. |

[↑ Back to top](#command-overview)

### `vsd extract`

Extract subtitles from a fragmented mp4 file

```
vsd extract [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: Fragmented mp4 file path containing either wvtt (webvtt) or stpp (ttml) boxes.

For multiple segments, use merge sub-command to combine them into a single file first. *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `-c, --codec` | Codec for output subtitles<br>*Possible values:* `subrip`, `webvtt`<br>*Default:* `webvtt` |
| `-o, --output` | Output file path for the extracted subtitles.<br><br>The codec is inferred from the file extension (.srt or .vtt). |

[↑ Back to top](#command-overview)

### `vsd license`

Request content keys from a license server

```
vsd license [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: https://.. (playlist) | video-init.mp4 | pssh-data (base64) *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `-H, --header` | Additional headers for license request in same format as curl.<br><br>This option can be used multiple times. |

**Playready Options:**

| Flag | Description |
|------|-------------|
| `--playready-device` | Path to the playready device (.prd) file.<br><br>To create a .prd file, see https://pypi.org/project/pyplayready |
| `--playready-url` | Playready license server url |
| `--skip-playready` | Skip playready license request |

**Widevine Options:**

| Flag | Description |
|------|-------------|
| `--widevine-device` | Path to the widevine device (.wvd) file.<br><br>To create a .wvd file, see https://pypi.org/project/pywidevine |
| `--widevine-url` | Widevine license server url |
| `--skip-widevine` | Skip widevine license request |

[↑ Back to top](#command-overview)

### `vsd merge`

Merge multiple media segments into a single file

```
vsd merge [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: List of input files e.g. *.ts, segment_*.m4s, etc *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `-o, --output` | Output file path for the merged file |
| `-t, --type` | Merge strategy to use.<br><br>binary: raw byte concatenation. ffmpeg: use concat demuxer for container aware merging.<br>*Possible values:* `binary`, `ffmpeg`<br>*Default:* `binary` |

[↑ Back to top](#command-overview)

### `vsd save`

Download streams from DASH or HLS playlist

```
vsd save [OPTIONS] <INPUT>
```

**Arguments:**

- `<INPUT>`: https://.. (playlist) | .m3u8 | .mpd *(required)*

**Options:**

| Flag | Description |
|------|-------------|
| `--base-url` | Baseurl for resolving relative segment paths for local playlist |
| `-o, --output` | Output file path for the muxed file using ffmpeg.<br><br>This will overwrite existing output file and delete downloaded streams. |
| `--parse` | Output playlist metadata as json instead of downloading streams |
| `--subs-codec` | Force a specific subtitle codec for muxing<br>*Default:* `copy` |

**Client Options:**

| Flag | Description |
|------|-------------|
| `--cookies` | Cookies file path for requests (netscape cookie file) |
| `-H, --header` | Additional headers for requests in same format as curl.<br><br>This option can be used multiple times. |
| `--proxy` | Proxy server url (http, https, or socks) |
| `--query` | Additional query parameters for requests |

**Decrypt Options:**

| Flag | Description |
|------|-------------|
| `--keys` | Decryption keys for drm protected content in hex format |
| `--no-decrypt` | Disable decryption and download encrypted streams |

**Download Options:**

| Flag | Description |
|------|-------------|
| `--clip` | Download a specific section of the stream (not accurate clipping).<br><br>Accepts time values in HH:MM:SS.SS, MM:SS.SS, or SS.SS formats. |
| `-d, --directory` | Directory path for downloaded streams |
| `--no-merge` | Disable segments merging |
| `--no-resume` | Disable resume and force re-downloading |
| `--retries` | Maximum retry attempts per segment<br>*Default:* `10` |
| `-t, --threads` | Maximum number of concurrent download threads (1–16)<br>*Default:* `5` |

**Format Selection Options:**

| Flag | Description |
|------|-------------|
| `-F, --list-formats` | List available streams in a table format |
| `--list-formats-json` | List available streams metadata as json |
| `-f, --format` | Format expression for selecting streams.<br><br>Visit https://clitic.github.io/vsd/usage/#format-selection for more info.<br>*Default:* `b+s+allund` |
| `-i, --interactive` | Enable interactive stream selection menu with styled prompts |
| `-I, --interactive-raw` | Enable interactive stream selection menu with plain text prompts |

[↑ Back to top](#command-overview)

