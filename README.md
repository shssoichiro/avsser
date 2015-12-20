# avsser

## Purpose
Avsser is a command-line utility for generating Avisynth scripts. It is written
in Rust, primarily because I'm more familiar with Rust than with Python.

Currently Avsser can take a video file or directory containing videos and create
an Avisynth script with the same file name (extension changed to .avs) that loads
the video with `FFVideoSource`.

I hope to add the following features as well:

- [ ] Recursively scan directories
- [x] Allow optionally sourcing audio with video
- [x] Automatically extract subtitles from Matroska containers
  - [ ] Support choosing which subtitle track to export, if multiple available
  - [x] Extract fonts from Matroska containers
  - [ ] Automatically install extracted fonts on the user's system
- [ ] Detect ordered chapters and automatically link videos in generated script
- [ ] Allow selection of filters to automatically apply to all files during a run

## Dependencies

Certain features require ffmpeg and mkvtoolnix to be installed on your system.

## Versioning

Avsser uses [Semantic Versioning](http://semver.org/) for all of its releases.

## License

Avsser is released under the MIT License.

## Compiling

Use `cargo build --release`.

## Contributing

All contributions are welcome via Github. Bug reports and feature requests should
be submitted via the Issues tab. If you can include a pull request, that is helpful
but not necessary. I appreciate bug reports from any users regardless of coding
ability.
