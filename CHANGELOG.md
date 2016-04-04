# Changelog

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
