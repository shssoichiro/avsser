# Changelog

## 0.7.3
- Fix more bugs in `--120` option related to `canonicalize` function

## 0.7.2
* Fix bug in `--120` option related to relative paths 

## 0.7.1
* Remove dependency on rustc-serialize

## 0.7.0
* Add `--120` option to convert VFR to 120fps CFR

## 0.6.5
* Use Spline64Resize instead of Lanczos

## 0.6.4
* Fix bug where fonts with uppercase extensions would not be extracted

## 0.6.3
* Bump regex crate to 0.2
* Performance optimizations

## 0.6.2
* Error on running external commands instead of silently failing
* Use lazy_static on regexes for small performance increase

## 0.6.1
* Fix bug where AVS script would reference wrong path for input files

## 0.6.0
* -T is a new flag which can select which subtitle track to extract (defaults to 0)

## 0.5.0
* Flag to not add remove grain
* Flag to resize video (using lanczos4resize)
* Run clippy on everything

## 0.4.2
* Don't ask to overwrite subtitles a million times when writing ordered chapters (#1)

## 0.4.1
* Recognize .dga and .d2v files as valid input formats

## 0.4.0
* Automatically join ordered chapters in our script

## 0.3.0
* Can add audio to scripts, from video or from file with same name, different extension
* Can extract fonts from MKV containers
* Minor optimizations and code cleanup

## 0.2.1

* Remove clippy dependency (run it through cargo instead)
* Avsser should now compile in the latest stable Rust (1.5.0)

## 0.2.0

* Extract first subtitle track from a video with -S
* TextSub subtitle file (filename.ass) into the output script with -s

## 0.1.1

* Bugfix for Avisynth scripts sourcing themselves when running the program twice

## 0.1.0

* Initial release
* Ability to create a basic Avisynth script for one or all files in a directory
