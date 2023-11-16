# audiowarden

audiowarden runs in the background and automatically blocks Spotify songs that you
don't want to hear anymore.

You simply add all songs that you don't want to hear into a Spotify playlist. audiowarden then runs in the background
and checks if the song played by Spotify is included in that playlist. If that is the case, audiowarden skips the song
and immediately plays the next song in your Spotify queue.

This can be useful if you have made the experience that Spotify keeps recommending a certain song to you, even
though you have heard it often enough (or you have never liked that song in the first place).


## Requirements

audiowarden was tested on **Linux only**. Since it is using D-Bus and MPRIS, it won't run
on other operating systems like Windows and Mac OS X.

I expect audiowarden to run on every Linux distribution. Feel free to open an issue if you can't get it to run
on your Linux distribution.

I have not tested audiowarden on ARM devices, but I expect it to run on ARM as well. However, you will need to compile
it yourself, because only x86 binaries are available at the release page at the moment.

## Current Status

This branch refers to the alpha version of audiowarden which is not yet ready for public use. The client-id used
by audiowarden is still in development mode and currently pending Spotify's approval. Until then, you won't
be able to use this application, unless you create your own Spotify Client ID and replace the client id included
in audiowarden.


### How to use it

1. Clone the code and build it via cargo: 
    ```
    cargo build --release
    ```
2. Create a new Spotify playlist (the name of that playlist does not matter).
3. Add songs that you want to block into that playlist.
4. Modify the description of that playlist: The description must include the following keyword:
    ```
   audiowarden:block_songs
   ```
5. Start audiowarden:
    ```
    RUST_LOG=info ./target/release/audiowarden
    ``` 
6. Visit the following URL in your browser. It will redirect you to Spotify in order to authorize audiowarden to
   access your playlists:
   http://localhost:7185/authorize_audiowarden

That's all. You can now test if audiowarden works as intended by playing a song that is included in a playlist
that includes the `audiowarden:block_songs` keyword. Spotify should immediately skip this song and play the next song
in your queue.


### Bugs, Questions, Feedback & Suggestions

If you found bug, please open a new [issue](https://github.com/nroi/audiowarden/issues).

If you have questions, feedback or want to have some feature implemented, please use the 
[discussions page](https://github.com/nroi/audiowarden/discussions) instead.

For Bugfixes and smaller improvements, Pull Requests are always welcome, but before you invest too much time
implementing a feature or some other improvement, please create a new thread at the
[discussions page](https://github.com/nroi/audiowarden/discussions) first to clarify if what you're planning
to build is actually desired.
