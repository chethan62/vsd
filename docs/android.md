---
icon: fontawesome/brands/android
---

# Android Support

1. Install [Termux](https://f-droid.org/en/packages/com.termux) app on your device, then enable storage permissions manually from its settings page. After that, run the following commands in the terminal.

    ```bash
    pkg update
    pkg upgrade
    pkg install ffmpeg
    ln -s /storage/emulated/0/Download Download
    ```

2. Install [vsd in termux](https://clitic.github.io/vsd/build/#termux). Currently, only `arm64-v8a` binaries pre-builts are available which can be installed using this command.

    ```bash
    curl -L https://github.com/clitic/vsd/releases/download/vsd-0.4.3/vsd-0.4.3-aarch64-linux-android.tar.xz | tar xJC $PREFIX/bin
    ```

3. Use third party browsers like [Lemur Browser](https://play.google.com/store/apps/details?id=com.lemurbrowser.exts) (*developer tools*) or [Via Browser](https://play.google.com/store/apps/details?id=mark.via.gp) (*tools > resource sniffer*) to find playlists within the websites.

4. Now run `vsd` command as usual, the streams would be directly downloaded in your android's downloads folder.  

    ```bash
    cd Download
    vsd save -o video.mp4 https://..
    ```
