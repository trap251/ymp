# ymp

YouTube Media Player. Browse and play YouTube music/audio from the terminal.

## Important!

This is not like a normal music player. Searching and Playing are not instant as it needs to fetch results for search and then use mpv to stream directly with the video's link. This doesn't store anything locally, so far. I needed this tool personally so I published it, in case someone else needs it as well <3

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

I use arch (btw), but this should work on any linux distro.

## Controls

### Playback (common mpv controls)

    9/0     |   Decrease/Increase Volume
    Space   |   Play/Pause
    <-/->   |   Seek Backward/Forward
    Escape  |   Stop

### Navigation

    j/k     |   Scroll
    H/L     |   Switch Tab
    /       |   Search
    Enter   |   Play Video
    m       |   Change Playback Mode \[Video/Audio\]
    C       |   Clear Queue

## Screenshots

![YouTube Media Player](https://github.com/trap251/ymp/blob/main/screenshots/ymp.png)
