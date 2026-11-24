//! Stage 2 (homr `segmentation/inference_segnet`): run the segmentation net
//! over 320x320 tiles and stitch the class map into five masks.
//!
//! A port of homr 0.7.1 `inference_segnet.inference` / `merge_patches` with
//! the arguments `main.get_predictions` uses (batch 8, step 320, window
//! 320). The quirks are kept on purpose: the last row and column of tiles
//! are pulled back inside the image, so they overlap their neighbours, and
//! where tiles overlap homr averages the class *numbers* and truncates
//! (classes 2 and 5 give 3). `get_predictions` then resizes the images to
//! the mask size, which is the same size, so nothing changes there.
use half::f16;
use ort::value::Tensor;

use crate::models::model_err;
use crate::{Error, Gray, Models, Predictions};

/// Tile size of the net (`win_size`).
pub const WIN: usize = 320;
/// Distance between tiles (`get_predictions` passes `step_size=320`).
pub const STEP: usize = 320;
/// Tiles per ONNX run (`extract(batch_size=8)`).
pub const BATCH: usize = 8;
/// Output classes: 0 background, 1 stems/rests, 2 noteheads, 3 clefs/keys,
/// 4 staff, 5 symbols.
pub const CLASSES: usize = 6;

/// Top-left corners of the tiles, in homr's order (rows, then columns).
/// A corner can be negative when the image is smaller than a tile.
pub fn tile_origins(width: usize, height: usize) -> Vec<(isize, isize)> {
    let (w, h) = (width as isize, height as isize);
    let win = WIN as isize;
    let mut out = Vec::new();
    for y_loop in (0..h.max(win)).step_by(STEP) {
        let y = y_loop.min(h - win);
        for x_loop in (0..w.max(win)).step_by(STEP) {
            out.push((x_loop.min(w - win), y));
        }
    }
    out
}

/// homr `extract_patch`: the 3x320x320 tile at (x, y) as fp16, grey copied
/// into all three channels (`cv2.COLOR_GRAY2BGR`), white outside the image.
fn extract_patch(image: &Gray, x: isize, y: isize, out: &mut [f16]) {
    let plane = WIN * WIN;
    let white = f16::from_f32(255.0);
    out[..3 * plane].fill(white);
    let (x0, y0) = (x.max(0) as usize, y.max(0) as usize);
    let x1 = ((x + WIN as isize) as usize).min(image.width);
    let y1 = ((y + WIN as isize) as usize).min(image.height);
    for (py, sy) in (y0..y1).enumerate() {
        let row = &image.data[sy * image.width + x0..sy * image.width + x1];
        for (px, &v) in row.iter().enumerate() {
            let v = f16::from_f32(v as f32);
            for c in 0..3 {
                out[c * plane + py * WIN + px] = v;
            }
        }
    }
}

/// `np.argmax(out, axis=0)` of one (6, 320, 320) output: first maximum wins.
fn argmax(out: &[f16], tile: &mut [u8]) {
    let plane = WIN * WIN;
    for (p, t) in tile.iter_mut().enumerate().take(plane) {
        let mut best = 0;
        let mut v = out[p];
        for c in 1..CLASSES {
            let o = out[c * plane + p];
            if o > v {
                v = o;
                best = c;
            }
        }
        *t = best as u8;
    }
}

/// homr `merge_patches`: put the tiles' class maps back together. Where
/// tiles overlap, the class numbers are averaged and truncated, as homr's
/// float sum, division and `astype(int64)` do.
pub fn merge_tiles(tiles: &[Vec<u8>], width: usize, height: usize) -> Gray {
    let mut sum = vec![0u16; width * height];
    let mut weight = vec![0u8; width * height];
    let origins = tile_origins(width, height);
    assert_eq!(origins.len(), tiles.len(), "one class map per tile");
    for (&(x, y), tile) in origins.iter().zip(tiles) {
        let (x0, y0) = (x.max(0) as usize, y.max(0) as usize);
        let x1 = ((x + WIN as isize) as usize).min(width);
        let y1 = ((y + WIN as isize) as usize).min(height);
        for (py, sy) in (y0..y1).enumerate() {
            for (px, sx) in (x0..x1).enumerate() {
                let i = sy * width + sx;
                sum[i] += tile[py * WIN + px] as u16;
                weight[i] += 1;
            }
        }
    }
    Gray {
        width,
        height,
        data: sum
            .iter()
            .zip(&weight)
            .map(|(&s, &w)| (s / w.max(1) as u16) as u8)
            .collect(),
    }
}

/// The five masks (0 or 1) of a merged class map.
pub fn masks(classes: &Gray) -> Predictions {
    let mask = |c: u8| Gray {
        width: classes.width,
        height: classes.height,
        data: classes.data.iter().map(|&v| (v == c) as u8).collect(),
    };
    Predictions {
        stems_rest: mask(1),
        notehead: mask(2),
        clefs_keys: mask(3),
        staff: mask(4),
        symbols: mask(5),
    }
}

/// Run segnet over the preprocessed image: the merged class map (0..5).
pub fn classify(models: &mut Models, image: &Gray) -> Result<Gray, Error> {
    if image.width == 0 || image.height == 0 {
        return Err(Error::Image("empty image".into()));
    }
    let origins = tile_origins(image.width, image.height);
    let per_tile = 3 * WIN * WIN;
    let mut tiles = Vec::with_capacity(origins.len());
    for batch in origins.chunks(BATCH) {
        let mut input = vec![f16::ZERO; batch.len() * per_tile];
        for (i, &(x, y)) in batch.iter().enumerate() {
            extract_patch(image, x, y, &mut input[i * per_tile..(i + 1) * per_tile]);
        }
        let x = Tensor::from_array(([batch.len(), 3, WIN, WIN], input)).map_err(model_err)?;
        let out = models
            .segnet
            .run(ort::inputs!["input" => x])
            .map_err(model_err)?;
        let (shape, y) = out["output"]
            .try_extract_tensor::<f16>()
            .map_err(model_err)?;
        let want = [batch.len() as i64, CLASSES as i64, WIN as i64, WIN as i64];
        if shape.as_ref() != want {
            return Err(Error::Model(format!("segnet output shape {shape:?}")));
        }
        let per_out = CLASSES * WIN * WIN;
        for i in 0..batch.len() {
            let mut tile = vec![0u8; WIN * WIN];
            argmax(&y[i * per_out..(i + 1) * per_out], &mut tile);
            tiles.push(tile);
        }
    }
    Ok(merge_tiles(&tiles, image.width, image.height))
}

/// Stage 2: the five masks at the preprocessed image's size, before homr's
/// `filter_predictions`.
pub fn segment(models: &mut Models, image: &Gray) -> Result<Predictions, Error> {
    Ok(masks(&classify(models, image)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_homr_segnet_tiles_like_python() {
        // homr: y = min(y_loop, h - win) over range(0, max(h, win), step).
        assert_eq!(tile_origins(320, 320), vec![(0, 0)]);
        assert_eq!(
            tile_origins(700, 330),
            vec![(0, 0), (320, 0), (380, 0), (0, 10), (320, 10), (380, 10)]
        );
        // Smaller than a tile: negative origin, padded with white.
        assert_eq!(tile_origins(100, 50), vec![(-220, -270)]);
        // The Lacrymosa page: 6 columns x 9 rows.
        assert_eq!(tile_origins(1920, 2711).len(), 54);
    }

    #[test]
    fn lib_06_homr_segnet_patch_is_white_outside() {
        let mut img = Gray::new(3, 2);
        img.data = vec![1, 2, 3, 4, 5, 6];
        let mut p = vec![f16::ZERO; 3 * WIN * WIN];
        extract_patch(&img, -317, -318, &mut p);
        // Image pixels are copied at the top-left (homr pads after them).
        assert_eq!(p[0].to_f32(), 1.0);
        assert_eq!(p[WIN + 2].to_f32(), 6.0);
        assert_eq!(p[2 * WIN * WIN + WIN].to_f32(), 4.0);
        assert_eq!(p[3].to_f32(), 255.0);
        assert_eq!(p[2 * WIN].to_f32(), 255.0);
    }

    #[test]
    fn lib_06_homr_segnet_argmax_first_max_wins() {
        let plane = WIN * WIN;
        let mut out = vec![f16::ZERO; CLASSES * plane];
        out[2 * plane] = f16::from_f32(1.0);
        out[4 * plane] = f16::from_f32(1.0); // tie: class 2 wins
        out[5 * plane + 1] = f16::from_f32(0.5);
        let mut t = vec![9u8; plane];
        argmax(&out, &mut t);
        assert_eq!((t[0], t[1], t[2]), (2, 5, 0));
    }

    #[test]
    fn lib_06_homr_segnet_overlap_truncates_mean_class() {
        // 330 wide: tiles at x 0 and 10 overlap on x 10..320.
        let (w, h) = (330, 320);
        let mut a = vec![2u8; WIN * WIN];
        let b = vec![5u8; WIN * WIN];
        a[0] = 4;
        let m = merge_tiles(&[a, b], w, h);
        assert_eq!(m.get(0, 0), 4); // only tile a
        assert_eq!(m.get(5, 7), 2);
        assert_eq!(m.get(10, 0), 3); // (2 + 5) / 2 = 3.5 -> 3
        assert_eq!(m.get(329, 0), 5); // only tile b
        let p = masks(&m);
        assert_eq!(p.clefs_keys.get(10, 0), 1);
        assert_eq!(p.notehead.get(5, 7), 1);
        assert_eq!(p.staff.get(0, 0), 1);
        assert_eq!(p.symbols.get(329, 0), 1);
        assert_eq!(
            p.stems_rest.data.iter().map(|&v| v as usize).sum::<usize>(),
            0
        );
    }
}
