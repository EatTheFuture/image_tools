# Changelog


## [Unreleased]

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


[Unreleased]: https://github.com/EatTheFuture/image_tools/compare/v0.2.1...HEAD
[0.2.0]: https://github.com/EatTheFuture/image_tools/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/EatTheFuture/image_tools/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EatTheFuture/image_rools/release/tag/v0.1.0