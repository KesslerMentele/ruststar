//! The public-facing surface of `qr_code`: everything a caller needs to build and render a
//! QR code, without reaching into the encoding internals (segmenting, bit packing, error
//! correction, masking) that the rest of the module handles.

pub(crate) use super::{
    create_qr_code, create_qr_code_bits,
    EncodedData, EncodingMode, ErrorCorrectionLevel,
    QRCreationError, QRVersion, QRVersionError, VersionChoice,
};
pub(crate) use super::structure::QRMatrix;
