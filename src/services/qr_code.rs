use bitvec::prelude::*;

pub mod penalty_score;
pub mod gf_math;
pub mod encoder;
pub mod bits;
pub mod segmenter;
mod capacity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QRVersionError {
    OutOfBounds(u8)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QRVersion {
    version: u8,
}

impl TryFrom<u8> for QRVersion {
    type Error = QRVersionError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value >= 1 && value <= 40 {Ok(QRVersion{version:value})} else {Err(QRVersionError::OutOfBounds(value)) }
    }
}

pub enum EncodingMode {
    Numeric,
    Alphanumeric,
    Byte,
    Kanji,
    ECI,
    FNC1,
    SA,
    AutoDetect,
    Segmented
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCorrectionLevel {
    L,
    M,
    Q,
    H
}


fn create_qr_code_bits(
    version:QRVersion,
    error_correction_level:ErrorCorrectionLevel,
    encoding_mode: Option<EncodingMode>
) -> Result<BitVec<u8, Lsb0>, QRVersionError> {


    todo!()
}