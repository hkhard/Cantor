//! The OpenCV operations homr uses, in plain Rust (no OpenCV dependency):
//! threshold, morphology (dilate/erode/open/close with rect and ellipse
//! kernels), connected components and contours, min-area rectangles,
//! resize (linear, cubic, nearest), remap, CLAHE. Each one is tested
//! against OpenCV's output on small images.
