# ymp

YouTube Media Player. Browse and play YouTube music/audio from the terminal.

## Installation (Linux)

Run this command:

```
curl -fL "https://github.com/trap251/ymp/releases/latest/download/ymp" -o /tmp/ymp \
  && sudo install -m 755 /tmp/ymp /usr/local/bin/ymp
```

Or manually:

1. Download the latest ymp binary from [releases](https://github.com/trap251/ymp/releases)
2. Place it in the binaries folder i.e. ~/.local/bin/ or /usr/local/bin/
3. Run 'ymp' in the terminal to use the application

#### Make sure to have the following dependencies installed:

1. yt-dlp (For YouTube search)
2. mpv (For media playback)

I use arch btw, but this should work on any linux distro.

## Controls

### Playback (common mpv controls)

    9/0     |   Decrease/Increase Volume
    Space   |   Play/Pause
    <-/->   |   Seek Backward/Forward
    Escape  |   Stop

## Screenshots

![YouTube Media Player](https://github.com/trap251/ymp/blob/main/screenshots/ymp.png)
