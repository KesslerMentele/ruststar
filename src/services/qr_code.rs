use bitvec::prelude::*;
use crate::services::qr_code::segmenter::{best_single_mode, segment, SegmentError, SegmentPlan};

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


/// How the caller wants the version chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionChoice {
    /// Use exactly this version, failing if the data doesn't fit.
    Fixed(QRVersion),
    /// Use the smallest version that fits the optimally segmented data.
    Auto,
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

pub enum QRCreationError {
    InvalidQRVersion(QRVersionError),
    NotSegmentable(SegmentError),
}

impl From<SegmentError> for QRCreationError {
    fn from(value: SegmentError) -> Self { QRCreationError::NotSegmentable(value) }
}
impl From<QRVersionError> for QRCreationError {
    fn from(value: QRVersionError) -> Self { QRCreationError::InvalidQRVersion(value) }
}

fn create_qr_code_bits(
    data: &str,
    error_correction_level:ErrorCorrectionLevel,
    encoding_mode: Option<EncodingMode>,
    version:Option<QRVersion>,
) -> Result<BitVec<u8, Lsb0>, QRCreationError> {
    let version_choice = match version {
        Some(version) => VersionChoice::Fixed(version),
        None => VersionChoice::Auto
    };
    match encoding_mode {
        Some(EncodingMode::Segmented) => {
            let segment_plan = segment(data,version_choice, error_correction_level)?;
        },
        Some(EncodingMode::AutoDetect) => {
            let mode = best_single_mode(data);
        }
    }




    todo!()
}