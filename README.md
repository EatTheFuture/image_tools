# Image Tools

A collection of tools useful when dealing with the images and colors from cameras in a VFX pipeline.

Currently there are only two tools: HDRI Merge and Camera Analyzer.


## HDRI Merge

HDRI Merge is a tool for merging a series of low-dynamic-range images at different exposures into a single high-dynamic-range image.

At the moment HDRI Merge only works correctly with image files that contain Exif data about their exposures.  Typically these will be JPEGs, although several standard image formats are supported.  The resulting HDRIs are always saved in .hdr format.

### TODO:

- [ ] Let user select a "look" to use for HDRI preview display (currently just applies sRGB gamma).
- [ ] Let user load and use custom LUTs to linearize and color-transform input images before merging (currently linearization is estimated, and there's no way to transform chroma before merging).
- [ ] Support camera raw images as input.
- [ ] Support saving to EXR files.


## LUT Maker

**WORK IN PROGRESS:** this tool is still very much an R&D project and is not ready for real use yet.  You're more than welcome to tinker with it, but don't expect it to be useful or correct yet.

LUT Maker will be a tool for creating LUT files for various purposes.  The initial focus will be analyzing and characterizing cameras, the results of which can then be exported as LUTs.  The MVP for a first release will focus on LUTs for linearizing input footage and calibrating black levels, and will likely not include any functionality related to chroma.

### TODO:

- [ ] Everything.


# License

This project is licensed under the GNU General Public License, version 3.  Please see LICENSE.md for details.

The EMoR basis curves are from the paper "Modeling the Space of Camera Response Functions" by Grossberg and Nayar, 2004, and can be found at [https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php](https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php).


# Contributing

Although we are not specifically looking for contributions right now, if you would like to contribute please keep in mind the following things:

- By submitting any work for inclusion in this project, you agree to license it under the same terms as above, and you assert that you have the rights to do so.
- Larger changes are likely to be rejected unless by pure coincidence they happen to align with our goals for the project.  So if you are considering a larger change, please either file an issue or otherwise contact us to discuss your idea before starting work on it, so that you don't inadvertantly waste your time.
