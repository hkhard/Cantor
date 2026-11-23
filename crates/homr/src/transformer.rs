//! Stage 6 (homr `transformer/*`): encoder on the staff strip, greedy
//! decoder with KV cache, tokens to homr's symbol strings.
use crate::{Error, Gray, Models, Symbol};

pub fn read_staff(models: &mut Models, strip: &Gray) -> Result<Vec<Symbol>, Error> {
    let _ = (models, strip);
    todo!("port staff2score, encoder_inference, decoder_inference, vocabulary")
}
