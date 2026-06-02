---
icon: lucide/house
---

# VSD

**V**ideo **S**tream **D**ownloader is a command-line utility and rust library for downloading streams from [DASH](https://en.wikipedia.org/wiki/Dynamic_Adaptive_Streaming_over_HTTP) `.mpd` manifests and [HLS](https://en.wikipedia.org/wiki/HTTP_Live_Streaming) `.m3u8` playlists.

<div align="center">
  <img src="https://raw.githubusercontent.com/clitic/vsd/refs/heads/main/docs/images/showcase.gif" width="700px">
</div>

## Features

- [x] **DASH & HLS Support**: Supports both DASH (`.mpd`) manifests and HLS (`m3u8`) playlists.
- [x] **DRM Support**: Decrypt protected content using keys acquired from Widevine and PlayReady license servers.
- [x] **Subtitles Extraction**: Extract subtitle tracks from fragmented mp4 streams.
- [x] **Multi-threaded Downloads**: Fetch media segments concurrently with customizable thread counts to maximize bandwidth.
- [x] **Rust Library Support**: Integrate `vsd` directly into your Rust projects as a library.
- [ ] **Live Stream Downloading**: Live stream downloading is not yet supported.
