# ETF Image Tools

Eat the Future's collection of tools for working with captured images and colors in a VFX pipeline.

Currently there are three tools: ETF HDRI Merge, ETF LUT Maker, and ETF OCIO Maker.

**This software is currently alpha quality.**  There is enough there to be useful, but don't expect a polished experience.  We also aren't able to test on all platforms, so you may run into issues depending on your hardware and OS.  Bug reports are very appreciated!


## ETF HDRI Merge

A tool for merging a series of low-dynamic-range images at different exposures into a single high-dynamic-range image.

At the moment this only works correctly with image files that contain Exif data about their exposures.  Typically these will be JPEGs, although several standard image formats are supported.  The resulting HDRIs are always saved in .hdr format.

### To-do:

- [ ] Let users select a filmic "look" when previewing the HDRI (currently it just maps straight to sRGB, which isn't great).
- [ ] Let users load custom transfer function LUTs (e.g. from ETF LUT Maker) to linearize input images.  Currently linearization is always estimated.
- [ ] Let users specify color gamut conversions.
- [ ] Support camera raw images as input, with demosaicing support.
- [ ] Support saving to EXR files.


## ETF LUT Maker

A tool to generate LUTs by analyzing purpose-captured images from a camera.

Currently this is only capable of generating transfer function LUTs.

### To-do:

- [x] Allow loading LUTs to be used as the basis for generating a new LUT (useful for e.g. correcting the black levels of a manufacturer-provided LUT).
- [x] Allow loading of 16-bit TIFF and PNG files, for estimating parameters of high bit-depth footage.
- [ ] Export transfer function LUTs with a bit of buffer outside the normal range of the LUT (useful for e.g. preserving negative values throughout a pipeline).
- [ ] Generate 3D LUTs for chroma, based on color checker images.
- [ ] Generate 3D LUTs for chroma, based on camera sensor spectral sensitivity data.


## ETF OCIO Maker

A tool to easily generate custom Open Color IO configurations.

Currently this only generates configurations based on the Blender 3.0 default configuration, but other configuration templates are planned.

### To-do:

- [x] Save data in the OCIO config file that allows OCIO Maker to re-open the config for further editing.  Right now, after closing OCIO Maker you have to start all over if you want to change something, which can be obnoxious.
- [x] Allow specifying custom chromaticity coordinates (currently limited to presets in a menu).
- [x] Allow specifying a base template config, along with custom reference/working color space.
- [x] Do reasonable gamut clipping on output transforms.
- [ ] Do reasonable gamut clipping on input transforms.
- [ ] Add a template for ACES "lite".  Essentially, the same as ACES except without the massive list of IDTs (since you'll be adding your own IDTs).
  - [x] "ACES Lite" template added, but not yet complete.
- [ ] Support for 3D LUTs.
- [ ] Allow enabling gamut mapping for OCIO 2.1 configs (once OCIO 2.1 is released).


# License

This project is licensed under the GNU General Public License, version 3.  Please see LICENSE.md for details.

The EMoR basis curves are from the paper "Modeling the Space of Camera Response Functions" by Grossberg and Nayar, 2004, and can be found at [https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php](https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php).


# Contributing

Although we are not specifically looking for contributions right now, if you would like to contribute please keep in mind the following things:

- By submitting any work for inclusion in this project, you agree to license it under the same terms as above, and you assert that you have the rights to do so.
- Larger changes are likely to be rejected unless by pure coincidence they happen to align with our goals for the project.  So if you are considering a larger change, please either file an issue or otherwise contact us to discuss your idea before starting work on it, so that you don't inadvertantly waste your time.
