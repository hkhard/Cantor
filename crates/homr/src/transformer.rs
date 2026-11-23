//! Stage 6 (homr `transformer/*`): encoder on the staff strip, greedy
//! decoder with KV cache, tokens to homr's symbol strings.
//!
//! Ports `staff2score.py` (`Staff2Score.predict`, `ConvertToArray`),
//! `encoder_inference.py`, `decoder_inference.py` (`ScoreDecoder.generate`)
//! and `vocabulary.py` ([`vocabulary`]).
pub mod vocabulary;

use std::borrow::Cow;

use ort::session::SessionInputValue;
use ort::value::{DynValue, Tensor};

use crate::models::model_err;
use crate::{Error, Gray, Models, Symbol};
pub use vocabulary::{EncodedSymbol, Vocabulary};

/// homr `Config`: strip size the encoder expects.
pub const MAX_HEIGHT: usize = 256;
pub const MAX_WIDTH: usize = 1280;
/// homr `Config.max_seq_len`.
pub const MAX_SEQ_LEN: usize = 608;
const BOS_TOKEN: i64 = 1;
const EOS_TOKEN: usize = 2;
const NONOTE_TOKEN: i64 = 0;
/// homr `Config`: decoder_depth 8, 4 cache tensors per layer, 8 heads of 64.
const CACHE_TENSORS: usize = 8 * 4;
const HEADS: usize = 8;
const HEAD_DIM: usize = 512 / HEADS;
const DIM: usize = 512;

/// homr `ConvertToArray`: pixels / 255, then (x - 0.7931) / 0.1738, computed
/// in f64 like numpy and stored as f32.
pub fn normalize(strip: &Gray) -> Vec<f32> {
    const MEAN: f64 = 0.7931;
    const STD: f64 = 0.1738;
    strip
        .data
        .iter()
        .map(|&p| ((p as f64 / 255.0 - MEAN) / STD) as f32)
        .collect()
}

/// homr `Encoder.generate`: the context, shape (1, n, 512), flattened.
pub fn encode(models: &mut Models, strip: &Gray) -> Result<(usize, Vec<f32>), Error> {
    let x = Tensor::from_array(([1usize, 1, strip.height, strip.width], normalize(strip)))
        .map_err(model_err)?;
    let out = models
        .encoder
        .run(ort::inputs!["input" => x])
        .map_err(model_err)?;
    let (shape, ctx) = out["output"]
        .try_extract_tensor::<f32>()
        .map_err(model_err)?;
    if shape.len() != 3 || shape[2] as usize != DIM {
        return Err(Error::Model(format!("encoder output shape {shape:?}")));
    }
    Ok((shape[1] as usize, ctx.to_vec()))
}

fn argmax(v: &[f32]) -> usize {
    // numpy argmax: the first index of the maximum.
    let mut best = 0;
    for (i, x) in v.iter().enumerate() {
        if *x > v[best] {
            best = i;
        }
    }
    best
}

/// homr `ScoreDecoder.generate`: greedy decoding with the KV cache, until
/// EOS or [`MAX_SEQ_LEN`] tokens.
pub fn decode(
    models: &mut Models,
    vocab: &Vocabulary,
    context_len: usize,
    context: &[f32],
) -> Result<Vec<EncodedSymbol>, Error> {
    let mut cache: Vec<DynValue> = (0..CACHE_TENSORS)
        .map(|_| {
            Tensor::from_array(([1usize, HEADS, 0, HEAD_DIM], Vec::<f32>::new()))
                .map(|t| t.into_dyn())
        })
        .collect::<Result<_, _>>()
        .map_err(model_err)?;
    let full = Tensor::from_array(([1usize, context_len, DIM], context.to_vec()))
        .map_err(model_err)?
        .into_dyn();
    // After the first step homr passes only context[:, :1].
    let reduced = Tensor::from_array(([1usize, 1, DIM], context[..DIM].to_vec()))
        .map_err(model_err)?
        .into_dyn();
    let cache_in: Vec<String> = (0..CACHE_TENSORS).map(|i| format!("cache_in{i}")).collect();
    let cache_out: Vec<String> = (0..CACHE_TENSORS)
        .map(|i| format!("cache_out{i}"))
        .collect();

    // Last tokens fed back: rhythm, pitch, lift, articulation, slur.
    let mut last = [
        BOS_TOKEN,
        NONOTE_TOKEN,
        NONOTE_TOKEN,
        NONOTE_TOKEN,
        NONOTE_TOKEN,
    ];
    let mut symbols = vec![];
    for step in 0..MAX_SEQ_LEN {
        let mut inputs: Vec<(Cow<str>, SessionInputValue)> = Vec::with_capacity(7 + CACHE_TENSORS);
        for (name, v) in ["rhythms", "pitchs", "lifts", "articulations", "slurs"]
            .iter()
            .zip(last)
        {
            let t = Tensor::from_array(([1usize, 1], vec![v])).map_err(model_err)?;
            inputs.push(((*name).into(), t.into()));
        }
        let ctx = if step == 0 { &full } else { &reduced };
        inputs.push(("context".into(), ctx.into()));
        let len = Tensor::from_array(([1usize], vec![step as i64])).map_err(model_err)?;
        inputs.push(("cache_len".into(), len.into()));
        for (name, c) in cache_in.iter().zip(&cache) {
            inputs.push((name.as_str().into(), c.into()));
        }
        let mut out = models.decoder.run(inputs).map_err(model_err)?;
        let pick = |name: &str| -> Result<usize, Error> {
            let (_, v) = out[name].try_extract_tensor::<f32>().map_err(model_err)?;
            Ok(argmax(v))
        };
        let rhythm = pick("out_rhythms")?;
        let pitch = pick("out_pitchs")?;
        let lift = pick("out_lifts")?;
        let position = pick("out_positions")?;
        let articulation = pick("out_articulations")?;
        let slur = pick("out_slurs")?;
        if rhythm == EOS_TOKEN {
            break;
        }
        let coordinates = {
            let (_, a) = out["attention"]
                .try_extract_tensor::<f32>()
                .map_err(model_err)?;
            (a.len() >= 2).then(|| (a[0], a[1]))
        };
        let token = |list: &[String], i: usize| -> Result<String, Error> {
            list.get(i)
                .cloned()
                .ok_or_else(|| Error::Model(format!("token {i} out of vocabulary")))
        };
        symbols.push(EncodedSymbol {
            rhythm: token(&vocab.rhythm, rhythm)?,
            pitch: token(&vocab.pitch, pitch)?,
            lift: token(&vocab.lift, lift)?,
            articulation: token(&vocab.articulation, articulation)?,
            slur: token(&vocab.slur, slur)?,
            position: token(&vocab.position, position)?,
            coordinates,
        });
        for (c, name) in cache.iter_mut().zip(&cache_out) {
            *c = out
                .remove(name.as_str())
                .ok_or_else(|| Error::Model(format!("decoder has no output {name}")))?;
        }
        last = [
            rhythm as i64,
            pitch as i64,
            lift as i64,
            articulation as i64,
            slur as i64,
        ];
    }
    Ok(symbols)
}

/// homr `Staff2Score.predict`: one dewarped staff strip (256 x 1280, as
/// homr's `staff_parsing` prepares it) to its symbols.
pub fn predict(models: &mut Models, strip: &Gray) -> Result<Vec<EncodedSymbol>, Error> {
    if strip.width != MAX_WIDTH || strip.height != MAX_HEIGHT {
        return Err(Error::Other(format!(
            "staff strip must be {MAX_WIDTH}x{MAX_HEIGHT}, got {}x{}",
            strip.width, strip.height
        )));
    }
    let (n, context) = encode(models, strip)?;
    decode(models, &Vocabulary::default(), n, &context)
}

/// Stage 6 for the pipeline: the symbols in homr's text form.
pub fn read_staff(models: &mut Models, strip: &Gray) -> Result<Vec<Symbol>, Error> {
    Ok(predict(models, strip)?
        .iter()
        .map(|s| s.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lib_06_homr_normalize_matches_convert_to_array() {
        let g = Gray {
            width: 3,
            height: 1,
            data: vec![0, 202, 255],
        };
        let v = normalize(&g);
        // numpy: ((np.array([0,202,255]) / 255) - 0.7931) / 0.1738
        assert!((v[0] - -4.563_291).abs() < 1e-5);
        assert!((v[1] - -0.005_426_566).abs() < 1e-5, "{}", v[1]);
        assert!((v[2] - 1.190_449).abs() < 1e-5);
    }

    #[test]
    fn lib_06_homr_argmax_takes_the_first_maximum_like_numpy() {
        assert_eq!(argmax(&[0.0, 2.0, 2.0, 1.0]), 1);
        assert_eq!(argmax(&[3.0]), 0);
    }
}
