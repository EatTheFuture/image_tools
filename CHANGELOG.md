# Changelog


## [Unreleased]

### New in LUT Maker
- Add a "linear" transfer function option, for when you only want to use it for black levels/noise floor.
- Add a "Bracketed Exposures Plot" view, to vizualize how well a transfer function linearizes colors.

### New in OCIO Maker
- Add Blackmagic Design's Wide Gamut Gen4/Gen5 color space chromaticities.
- Allow users to manually specify custom color space chromaticities.
- Can now re-open its own configs for further editing.

### Bug Fixes
- Opening file dialogs would sometimes crash on MacOS (issue #5).
- HDRI Merge wasn't accounting for sensor noise floors properly, which could lead to incorrectly noisy results in dark areas.

## [0.2.2] - 2021-12-21

### Bug Fixes

- Temporarily switch to git master of eframe, to get access to a fix for MacOS file dialogs.  Due to a bug in upstream libraries, opening a file dialog on MacOS would cause the whole program to freeze.  Once the fix makes it into a published version, we will switch back.


## [0.2.1] - 2021-12-17

### Bug Fixes

- "From linear" LUTs were not being written correctly from LUT Maker.
- OCIO Maker would freeze when trying to load a LUT file with non-finite values in it.


## [0.2.0] - 2021-12-16

### New tools.

- A LUT maker for estimating or generating transfer function LUTs.
- A OCIO config generator, for easily making custom OCIO configs with custom IDTs and ODTs.  Works well in combination with the LUT maker.  Currently only generates Blender-based configs.


## [0.1.0] - 2021-11-10

First release!  Includes a basic HDRI merging tool.


[Unreleased]: https://github.com/EatTheFuture/image_tools/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/EatTheFuture/image_tools/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/EatTheFuture/image_tools/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/EatTheFuture/image_tools/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EatTheFuture/image_rools/release/tag/v0.1.0