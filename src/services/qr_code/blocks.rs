use bitvec::prelude::*;
use crate::services::qr_code::{
    ErrorCorrectionLevel, QRVersion, bits::bits_to_bytes, 
    capacity::{ec_block_info, EcBlockInfo}, 
    gf_math::{error_correction_codewords, EcLength}
};

/// Slices `data` into its error-correction blocks (group 1's, then group 2's, preserving that
/// order since interleaving walks blocks in this order), borrowing from `data` rather than
/// copying it.
fn split_into_blocks(data: &[u8], info: EcBlockInfo) -> Vec<&[u8]> {
    let mut blocks = Vec::with_capacity((info.group_1_blocks + info.group_2_blocks) as usize);
    let mut rest = data;
    for _ in 0..info.group_1_blocks {
        let (block, remainder) = rest.split_at(info.group_1_data_codewords as usize);
        blocks.push(block);
        rest = remainder;
    }
    for _ in 0..info.group_2_blocks {
        let (block, remainder) = rest.split_at(info.group_2_data_codewords as usize);
        blocks.push(block);
        rest = remainder;
    }
    blocks
}

/// The error correction codewords for every block, flattened into one array: every block has
/// the same EC length, so block `i`'s codewords sit at `[i * ec_codewords_per_block ..][..ec_codewords_per_block]`.
fn generate_error_correction_bytes(blocks: &[&[u8]], ec_len: EcLength, ec_codewords_per_block: usize) -> Vec<u8> {
    let mut error_correction = Vec::with_capacity(blocks.len() * ec_codewords_per_block);
    for block in blocks {
        error_correction.extend(error_correction_codewords(block, ec_len));
    }
    error_correction
}

/// Builds the final, interleaved codeword sequence ready for placement: data codewords
/// column-wise across blocks, then error correction codewords the same way.
pub(crate) fn interleave(data: &BitSlice<u8, Lsb0>, version: QRVersion, error_correction_level: ErrorCorrectionLevel) -> BitVec<u8, Lsb0> {
    let data = bits_to_bytes(data);
    let data = data.as_slice();

    let info = ec_block_info(version, error_correction_level);
    let ec_codewords_per_block = info.ec_codewords_per_block as usize;
    let ec_len = EcLength::try_from(info.ec_codewords_per_block as u8)
        .expect("ec_codewords_per_block is always a valid EcLength");

    let blocks = split_into_blocks(data, info);
    let error_correction = generate_error_correction_bytes(&blocks, ec_len, ec_codewords_per_block);

    let total_bytes = data.len() + error_correction.len();
    let mut result = BitVec::with_capacity(total_bytes * 8);

    let max_data_len = blocks.iter().map(|block| block.len()).max().unwrap_or(0);
    for i in 0..max_data_len {
        for block in &blocks {
            if let Some(&byte) = block.get(i) {
                result.extend_from_bitslice(byte.view_bits::<Msb0>());
            }
        }
    }
    for i in 0..ec_codewords_per_block {
        for block_index in 0..blocks.len() {
            let byte = error_correction[block_index * ec_codewords_per_block + i];
            result.extend_from_bitslice(byte.view_bits::<Msb0>());
        }
    }

    result
}
