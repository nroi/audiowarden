# audiowarden

audiowarden runs in the background and automatically blocks Spotify songs that you
don't want to hear anymore.

You simply add all songs that you don't want to hear into a config file. audiowarden then runs in the background
and checks if the song played by Spotify is included in that list. If that is the case, audiowarden skips the song
and immediately plays the next song in your Spotify queue.

This can be useful if you have made the experience that Spotify keeps recommending a certain song to you, even
though you have heard it often enough (or you have never liked that song in the first place).


## Requirements

audiowarden was tested on **Linux only**. Since it is using D-Bus and MPRIS, it probably won't run
on other operating systems like Windows and Mac OS X.

I expect audiowarden to run on every Linux distribution. Feel free to open an issue if you can't get it to run
on your Linux distribution.

I have not tested audiowarden on ARM devices, but I expect it to run on ARM as well. However, you will need to compile
it yourself, because only x86 binaries are available at the release page at the moment.

## How-to use it

### Installation

#### Arch Linux
If you use ArchLinux, a package is available on the [AUR](https://aur.archlinux.org/packages/audiowarden-bin).

#### Other Linux Distributions

If you're using another Linux distribution, you can just fetch the binary from the release section,
or clone the code and build it via cargo:
```
cargo build --release
```

If you want to use systemd to have audiowarden run in the background, you can fetch a systemd service
user from the [AUR](https://aur.archlinux.org/cgit/aur.git/plain/audiowarden.service?h=audiowarden-bin).

### Run it
You can just execute the binary, but you will probably want to set up a systemd service to have it
running in the background. If you have installed audiowarden from the AUR, a systemd user service is
already available, and you just need to start and enable it:

```bash
systemctl --user start --now audiowarden.service
```

Remember to execute this command as your normal user, not as root.

Next, check the output of that systemd user service to verify it has successfully started:

```bash
journalctl -f --user-unit=audiowarden
```

Again, execute this command as your normal user, not as root.


Next, open the config file. In most cases, it will be stored in
`~/.config/audiowarden/blocked_songs.conf`.
If that file does not exist, check the output of the previously mentioned `journalctl` command:

```bash
journalctl -f --user-unit=audiowarden
```

The output should include a line like the following:

```
Configuration directory: /home/john-doe/.config/audiowarden
```

This shows you the directory which contains the file `blocked_songs.conf`.

The file includes an example entry for demonstration purposes: Each line in this file should contain a
URL that looks like this:

```
https://open.spotify.com/track/6CE6xXEI29e6X0noaNugIW
```

If the URL also includes the `si` query parameter, that does not matter, so something like this
is also acceptable:

```
https://open.spotify.com/track/6myHCyqMUCtqqsYZj9WZBR?si=6a1711d6e4a04265
```

### How to block songs

If you have a song playing in Spotify, and you want to block this, simply use the "share" functionality
to obtain its URL: For example, if you use the Spotify Desktop app, open the context menu of a song via Right-click
or by clicking on the three dots.
Next, click on "Share", and then on "Copy Song Link".

If you want to copy multiple URLs at once from the desktop app, simply use the shortcut Ctrl + C: For example, 
press Ctrl and leave it pressed while selecting all songs you want to block, then press Ctrl + C to copy the URLs
of all selected songs into your clipboard.

Add the URLs into the `blocked_songs.conf` file, with one URL per line. It is not required to restart audiowarden:
Once you play a new song, audiowarden will read the config file and pick up any changes you've made.

Alternatively, if the song that you want to block is currently playing, you can send a message via a Unix Domain
Socket, like so:
```bash
echo 'block_current_song' | nc -N -U "$XDG_RUNTIME_DIR/audiowarden/audiowarden.sock"
```

This requires netcat (`nc`) to be installed.

### Bugs, Questions, Feedback & Suggestions

If you found bug, please open a new [issue](https://github.com/nroi/audiowarden/issues).

If you have questions, feedback or want to have some feature implemented, please use the 
[discussions page](https://github.com/nroi/audiowarden/discussions) instead.

For Bugfixes and smaller improvements, Pull Requests are always welcome, but before you invest too much time
implementing a feature or some other improvement, please create a new thread at the
[discussions page](https://github.com/nroi/audiowarden/discussions) first to clarify if what you're planning
to build is actually desired.
