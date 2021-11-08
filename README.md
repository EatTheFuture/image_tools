# Image Tools

A collection of tools useful when dealing with the images and colors from cameras in a VFX pipeline.

Currently there are only two tools: HDRI Merge and Camera Analyzer.


## HDRI Merge

HDRI Merge is a tool for merging a series of low-dynamic-range images at different exposures into a single high-dynamic-range image.

At the moment HDRI merge only works correctly with image files that contain Exif data about their exposures.  Typically these will be JPEGs, although several standard image formats are supported.  The resulting HDRIs are always saved in .hdr format.

### TODO:

- [ ] Support camera raw images as input.
- [ ] Support saving to EXR files.
- [ ] Support loading and using custom transfer function LUTs to linearize input images (currently only automatic estimation is done, which works well but isn't 100% precise).
- [ ] Allow the user to specify sensor floor/ceiling values manually (currently they are always invisibly auto-detected).


## Camera Analyzer

**WORK IN PROGRESS:** this tool is still very much an R&D project and is not ready for real use yet.  You're more than welcome to tinker with it, but don't expect it to be useful or correct yet.

Camera Analyzer will be a tool for characterizing the luminance and chroma/spectral responses of cameras.  The initial focus of the tool will be luminance response, since that is both more important and far easier to tackle than chroma/spectral.

### TODO:

- [ ] Everything.


# License

This project is licensed under the GNU General Public License, version 3.  Please see LICENSE.md for details.

The EMoR basis curves are from the paper "Modeling the Space of Camera Response Functions" by Grossberg and Nayar, 2004, and can be found at [https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php](https://www.cs.columbia.edu/CAVE/software/softlib/dorf.php).


# Contributing

Although we are not specifically looking for contributions right now, if you would like to contribute please keep in mind the following things:

- By submitting any work for inclusion in this project, you agree to license it under the same terms as above, and you assert that you have the rights to do so.
- Larger changes are likely to be rejected unless by pure coincidence they happen to align with our goals for the project.  So if you are considering a larger change, please either file an issue or otherwise contact us to discuss your idea before starting work on it, so that you don't inadvertantly waste your time.
