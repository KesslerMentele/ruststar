use std::ops::{Add, Div, Mul, Not};

/// Antilog table for GF(256): `EXP_TABLE[i] = alpha^i`, where `alpha = 2` is the field's
/// generator element under the QR code's primitive polynomial (x^8 + x^4 + x^3 + x^2 + 1).
/// The multiplicative group has order 255, so `EXP_TABLE[255] == EXP_TABLE[0] == 1`.
const EXP_TABLE: [u8; 256] = [
    1, 2, 4, 8, 16, 32, 64, 128, 29, 58, 116, 232, 205, 135, 19, 38, 76, 152, 45, 90, 180, 117,
    234, 201, 143, 3, 6, 12, 24, 48, 96, 192, 157, 39, 78, 156, 37, 74, 148, 53, 106, 212, 181,
    119, 238, 193, 159, 35, 70, 140, 5, 10, 20, 40, 80, 160, 93, 186, 105, 210, 185, 111, 222, 161,
    95, 190, 97, 194, 153, 47, 94, 188, 101, 202, 137, 15, 30, 60, 120, 240, 253, 231, 211, 187,
    107, 214, 177, 127, 254, 225, 223, 163, 91, 182, 113, 226, 217, 175, 67, 134, 17, 34, 68, 136,
    13, 26, 52, 104, 208, 189, 103, 206, 129, 31, 62, 124, 248, 237, 199, 147, 59, 118, 236, 197,
    151, 51, 102, 204, 133, 23, 46, 92, 184, 109, 218, 169, 79, 158, 33, 66, 132, 21, 42, 84, 168,
    77, 154, 41, 82, 164, 85, 170, 73, 146, 57, 114, 228, 213, 183, 115, 230, 209, 191, 99, 198,
    145, 63, 126, 252, 229, 215, 179, 123, 246, 241, 255, 227, 219, 171, 75, 150, 49, 98, 196, 149,
    55, 110, 220, 165, 87, 174, 65, 130, 25, 50, 100, 200, 141, 7, 14, 28, 56, 112, 224, 221, 167,
    83, 166, 81, 162, 89, 178, 121, 242, 249, 239, 195, 155, 43, 86, 172, 69, 138, 9, 18, 36, 72,
    144, 61, 122, 244, 245, 247, 243, 251, 235, 203, 139, 11, 22, 44, 88, 176, 125, 250, 233, 207,
    131, 27, 54, 108, 216, 173, 71, 142, 1,
];
/// Discrete log table for GF(256), the inverse of `EXP_TABLE`: `LOG_TABLE[x] = i` such that
/// `alpha^i == x`, for `x` in `1..256`. `LOG_TABLE[0]` has no meaningful value since zero has
/// no discrete logarithm; it's never read for `x == 0` (callers check for that case first).
const LOG_TABLE: [u8; 256] =[
    0, 255, 1, 25, 2, 50, 26, 198, 3, 223, 51, 238, 27, 104, 199, 75, 4, 100, 224, 14, 52, 141,
    239, 129, 28, 193, 105, 248, 200, 8, 76, 113, 5, 138, 101, 47, 225, 36, 15, 33, 53, 147, 142,
    218, 240, 18, 130, 69, 29, 181, 194, 125, 106, 39, 249, 185, 201, 154, 9, 120, 77, 228, 114,
    166, 6, 191, 139, 98, 102, 221, 48, 253, 226, 152, 37, 179, 16, 145, 34, 136, 54, 208, 148,
    206, 143, 150, 219, 189, 241, 210, 19, 92, 131, 56, 70, 64, 30, 66, 182, 163, 195, 72, 126,
    110, 107, 58, 40, 84, 250, 133, 186, 61, 202, 94, 155, 159, 10, 21, 121, 43, 78, 212, 229, 172,
    115, 243, 167, 87, 7, 112, 192, 247, 140, 128, 99, 13, 103, 74, 222, 237, 49, 197, 254, 24,
    227, 165, 153, 119, 38, 184, 180, 124, 17, 68, 146, 217, 35, 32, 137, 46, 55, 63, 209, 91, 149,
    188, 207, 205, 144, 135, 151, 178, 220, 252, 190, 97, 242, 86, 211, 171, 20, 42, 93, 158, 132,
    60, 57, 83, 71, 109, 65, 162, 31, 45, 67, 216, 183, 123, 164, 118, 196, 23, 73, 236, 127, 12,
    111, 246, 108, 161, 59, 82, 41, 157, 85, 170, 251, 96, 134, 177, 187, 204, 62, 90, 203, 89, 95,
    176, 156, 169, 160, 81, 11, 245, 22, 235, 122, 117, 44, 215, 79, 174, 213, 233, 230, 231, 173,
    232, 116, 214, 244, 234, 168, 80, 88, 175,
];

/// One of the valid error-correction codeword counts a QR generator polynomial can be built
/// for. Constructible only via `TryFrom<u8>`, which rejects any length not in `ALL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EcLength(u8);

impl EcLength {
    /// Every valid EC codeword length, in the same order as `EC_POLYNOMIALS` below (so a
    /// length's index in `ALL` is also its index into `EC_POLYNOMIALS`).
    const ALL: [EcLength; 13] = [
        EcLength(7),
        EcLength(10),
        EcLength(13),
        EcLength(15),
        EcLength(16),
        EcLength(17),
        EcLength(18),
        EcLength(20),
        EcLength(22),
        EcLength(24),
        EcLength(26),
        EcLength(28),
        EcLength(30),
    ];

    /// Returns the EC length as a plain `usize`, the unit polynomial lengths and loop bounds
    /// elsewhere in this module are expressed in.
    fn value(self) -> usize {
        self.0 as usize
    }

    /// Returns the precomputed Reed-Solomon generator polynomial for this EC length, as
    /// coefficients in ascending degree order (`x^0 .. x^(t-1)`), matching
    /// `GaloisPolynomial`'s convention. `EC_POLYNOMIALS` is kept in the same order as `ALL`,
    /// so the lookup is just a position search.
    pub(crate) fn generator_polynomial_bytes(self) -> &'static [u8] {
        let index = Self::ALL
            .iter()
            .position(|&length| length == self)
            .expect("EcLength can only hold a value present in ALL");
        EC_POLYNOMIALS[index]
    }
}

impl TryFrom<u8> for EcLength {
    type Error = ();
    /// Builds an `EcLength` from a raw byte, succeeding only if it matches one of the lengths
    /// in `ALL`.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::ALL.into_iter().find(|&length| length.0 == value).ok_or(())
    }
}

/// Precomputed generator polynomial bytes for each length in `EcLength::ALL` (same
/// index order), coefficients in ascending degree order (x^0 .. x^(t-1))
const EC_POLYNOMIALS: [&[u8]; 13] = [
    &[117, 68, 11, 164, 154, 122, 127],
    &[193, 157, 113, 95, 94, 199, 111, 159, 194, 216],
    &[120, 132, 83, 43, 46, 13, 52, 17, 177, 17, 227, 73, 137],
    &[26, 134, 32, 151, 132, 139, 105, 105, 10, 74, 112, 163, 111, 196, 29],
    &[59, 36, 50, 98, 229, 41, 65, 163, 8, 30, 209, 68, 189, 104, 13, 59],
    &[79, 99, 125, 53, 85, 134, 143, 41, 249, 83, 197, 22, 119, 120, 83, 66, 119],
    &[146, 217, 67, 32, 75, 173, 82, 73, 220, 240, 215, 199, 175, 149, 113, 183, 251, 239],
    &[174, 165, 121, 121, 198, 228, 22, 187, 36, 69, 150, 112, 220, 6, 99, 111, 5, 240, 185, 152],
    &[245, 145, 26, 230, 218, 86, 253, 67, 123, 29, 137, 28, 40, 69, 189, 19, 244, 182, 176, 131, 179, 89],
    &[117, 144, 217, 127, 247, 237, 1, 206, 43, 61, 72, 130, 73, 229, 150, 115, 102, 216, 237, 178, 70, 169, 118, 122],
    &[94, 43, 77, 146, 144, 70, 68, 135, 42, 233, 117, 209, 40, 145, 24, 206, 56, 77, 152, 199, 98, 136, 4, 183, 51, 246],
    &[197, 58, 74, 176, 147, 121, 100, 181, 127, 233, 119, 117, 56, 247, 12, 167, 41, 100, 174, 103, 150, 208, 251, 18, 13, 28, 9, 252],
    &[150, 130, 106, 11, 195, 216, 74, 228, 46, 18, 25, 72, 246, 181, 51, 138, 217, 22, 177, 103, 70, 5, 98, 75, 192, 195, 73, 77, 246, 212],
];


/// A single element of GF(2^8), the finite field QR's Reed-Solomon error correction is defined
/// over. Addition is XOR; multiplication, inversion and division go through the
/// `EXP_TABLE`/`LOG_TABLE` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct GaloisFieldNumber(u8);
impl GaloisFieldNumber {
    /// Field addition. Addition and subtraction coincide in GF(2^n), so
    /// this is also effectively subtraction;
    pub(crate) fn wrapping_add(&self, p0: GaloisFieldNumber) -> GaloisFieldNumber {
        GaloisFieldNumber(self.0 ^ p0.0)
    }
}
impl From<u8> for GaloisFieldNumber {
    /// Wraps a raw byte as a field element.
    fn from(value: u8) -> Self {
        GaloisFieldNumber(value)
    }
}
impl Add for GaloisFieldNumber {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.wrapping_add(rhs)
    }
}
impl Mul for GaloisFieldNumber {
    type Output = Self;
    /// Field multiplication via the log/antilog tables: `a * b = alpha^(log(a) + log(b))`.
    /// Zero is handled separately since it has no logarithm.
    fn mul(self, rhs: Self) -> Self::Output {
        if self.0 == 0 || rhs.0 == 0 {
            return GaloisFieldNumber(0)
        }
        // The multiplicative group has order 255, so exponents must wrap mod 255.
        let log_sum = LOG_TABLE[self.0 as usize] as u16 + LOG_TABLE[rhs.0 as usize] as u16;
        GaloisFieldNumber(EXP_TABLE[(log_sum % 255) as usize])
    }
}
impl Not for GaloisFieldNumber {
    type Output = Self;
    /// Field multiplicative inverse (`a^-1`), computed as `alpha^(255 - log(a))`.
    fn not(self) -> Self {
        GaloisFieldNumber(EXP_TABLE[255u8.wrapping_sub(LOG_TABLE[self.0 as usize]) as usize])
    }
}
impl Div for GaloisFieldNumber {
    type Output = Self;
    /// Field division, implemented as multiplication by the divisor's inverse: `a / b = a * b^-1`.
    fn div(self, rhs: Self) -> Self {
        self * !rhs
    }
}

/// A polynomial over GF(2^8), stored as coefficients in ascending degree order: `self.0[0]`
/// is the constant term, `self.0[1]` the x^1 coefficient, and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GaloisPolynomial(Vec<GaloisFieldNumber>);
impl GaloisPolynomial {
    /// Returns the number of coefficients stored (one more than the degree, assuming no
    /// trailing zero coefficients have been trimmed).
    fn len(&self) -> usize {
       self.0.len()
    }

    /// Multiplies the polynomial by x^m.
    fn shift(self, m: usize) -> Self {
        let shifted = std::iter::repeat(GaloisFieldNumber::default())
            .take(m)
            .chain(self.0)
            .collect();
        GaloisPolynomial(shifted)
    }

    /// Reduces the polynomial mod x^n: keeps only the n lowest-degree coefficients (the terms
    /// of degree < n).
    fn truncate(mut self, n: usize) -> Self {
        self.0.truncate(n);
        self
    }

    /// Returns true if every coefficient is zero.
    fn is_zero(&self) -> bool {
        self.0.iter().all(|&coefficient| coefficient == GaloisFieldNumber::default())
    }

    /// Evaluates the polynomial at `x` using Horner's method.
    fn evaluate(&self, x: GaloisFieldNumber) -> GaloisFieldNumber {
        self.0
            .iter()
            .rev()
            .fold(GaloisFieldNumber::default(), |acc, &coefficient| acc * x + coefficient)
    }

    /// Returns the formal derivative of the polynomial over GF(2^n). Since the field has
    /// characteristic 2, differentiating x^i gives `i * x^(i-1)` reduced mod 2, which zeroes
    /// out every even-power term and keeps only the odd-power ones, shifted down one degree.
    fn derivative(&self) -> Self {
        if self.0.len() <= 1 {
            return GaloisPolynomial(Vec::new());
        }
        let result = (1..self.0.len())
            .map(|i| if i % 2 == 1 { self.0[i] } else { GaloisFieldNumber::default() })
            .collect();
        GaloisPolynomial(result)
    }
}
impl Add for GaloisPolynomial {
    type Output = Self;
    /// Polynomial addition: coefficient-wise addition (XOR), padding the shorter operand with
    /// zero coefficients.
    fn add(self, rhs: Self) -> Self::Output {
        let length = self.0.len().max(rhs.0.len());
        let result = (0..length)
            .map(|i| {
                let a = self.0.get(i).copied().unwrap_or_default();
                let b = rhs.0.get(i).copied().unwrap_or_default();
                a + b
            })
            .collect();
        GaloisPolynomial(result)
    }
}
impl Mul<GaloisPolynomial> for GaloisPolynomial {
    type Output = Self;
    /// Polynomial multiplication (convolution of coefficients), producing a result with
    /// `deg(self) + deg(rhs)` as its degree.
    fn mul(self, rhs: Self) -> Self::Output {
        if self.0.is_empty() || rhs.0.is_empty() {
            return GaloisPolynomial(Vec::new());
        }
        let mut result = vec![GaloisFieldNumber::default(); self.0.len() + rhs.0.len() - 1];
        for (i, &a) in self.0.iter().enumerate() {
            for (j, &b) in rhs.0.iter().enumerate() {
                result[i + j] = result[i + j] + (a * b);
            }
        }
        GaloisPolynomial(result)
    }
}
impl Mul<GaloisFieldNumber> for GaloisPolynomial {
    type Output = Self;
    /// Scales every coefficient by a single field element.
    fn mul(self, rhs: GaloisFieldNumber) -> Self::Output {
        GaloisPolynomial(self.0.into_iter().map(|coefficient| coefficient * rhs).collect())
    }
}


/// Computes the Reed-Solomon remainder of `data` modulo the EC generator polynomial for
/// `ec_len`, via synthetic division. This is the error-correction codeword appended after
/// `data` to form a decodable QR data block.
fn reed_solomon_remainder(data: GaloisPolynomial, ec_len: EcLength) -> GaloisPolynomial {
    let generator = ec_len.generator_polynomial_bytes();
    let t = generator.len();

    // Message polynomial = data(x) * x^t, so there's room for the remainder once divided.
    let mut poly = data.shift(t).0;

    // Synthetic division: cancel the leading term at each degree, walking down from the
    // top (len - 1) to t. Everything below t is what's left over once every degree >= t
    // has been cancelled - the remainder.
    for degree in (t..poly.len()).rev() {
        let coef = poly[degree];
        if coef == GaloisFieldNumber::default() {
            continue;
        }
        poly[degree] = GaloisFieldNumber::default(); // cancels the generator's implicit x^t term
        for (j, &g_j) in generator.iter().enumerate() {
            poly[degree - t + j] = poly[degree - t + j] + (coef * GaloisFieldNumber::from(g_j));
        }
    }

    poly.truncate(t);
    GaloisPolynomial(poly)
}
/// Evaluates `received` at each power of alpha used by `ec_len`'s generator polynomial,
/// producing the syndrome polynomial. All syndromes are zero iff the block decodes as
/// error-free.
fn compute_syndromes(received: &GaloisPolynomial, ec_len: EcLength) -> GaloisPolynomial {
    let syndromes = (0..ec_len.value())
        .map(|i| received.evaluate(GaloisFieldNumber::from(EXP_TABLE[i])))
        .collect();
    GaloisPolynomial(syndromes)
}
/// Runs the Berlekamp-Massey algorithm over `syndromes` to find the shortest linear feedback
/// shift register that generates them. The reciprocals of its roots mark the error positions within
/// the block.
fn berlekamp_massey(syndromes: GaloisPolynomial) -> GaloisPolynomial {
    let one = GaloisFieldNumber::from(1);
    let mut current_best_locator = GaloisPolynomial(vec![one]);
    let mut previous_best_locator = GaloisPolynomial(vec![one]);
    let mut error_count = 0usize;
    let mut current_processed_count = 1usize;
    let mut last_discrepancy_value = one;

    // syndromes and the locator are both ascending here (syndromes.0[k] = S_k, locator.0[i]
    // = coefficient of x^i directly), so unlike the TS version there's no index flip needed.
    for n in 0..syndromes.len() {
        let mut delta = syndromes.0[n];
        for i in 1..=error_count {
            delta = delta + (current_best_locator.0[i] * syndromes.0[n - i]);
        }

        if delta == GaloisFieldNumber::default() {
            current_processed_count += 1;
            continue;
        }

        let correction = previous_best_locator.clone().shift(current_processed_count)
            * (delta / last_discrepancy_value);
        let new_best_locator = current_best_locator.clone() + correction;

        if 2 * error_count <= n {
            previous_best_locator = current_best_locator;
            error_count = n + 1 - error_count;
            current_best_locator = new_best_locator;
            last_discrepancy_value = delta;
            current_processed_count = 1;
        } else {
            current_best_locator = new_best_locator;
            current_processed_count += 1;
        }
    }

    current_best_locator
}
/// Computes the error evaluator polynomial `Omega(x) = (S(x) * Lambda(x)) mod x^n`, used
/// together with the locator's derivative by `forney_magnitude` to recover error magnitudes.
fn error_evaluator(syndromes: GaloisPolynomial, locator: GaloisPolynomial) -> GaloisPolynomial {
    let n = syndromes.len();
    (syndromes * locator).truncate(n)
}
/// An error found by `chien_search`: the byte position it occurred at within the block, plus
/// the locator root's reciprocal ("error locator number") needed by `forney_magnitude`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocatedError {
    position: usize,
    locator_number: GaloisFieldNumber,
}
/// Searches for the roots of `locator` among the field elements `alpha^0 .. alpha^(block_length - 1)`
/// (the Chien search), returning one `LocatedError` per root found.
fn chien_search(locator: &GaloisPolynomial, block_length: usize) -> Vec<LocatedError> {
    (0..block_length)
        .filter_map(|j| {
            let locator_number = GaloisFieldNumber::from(EXP_TABLE[j]);
            if locator.evaluate(!locator_number) == GaloisFieldNumber::default() {
                Some(LocatedError { position: j, locator_number })
            } else {
                None
            }
        })
        .collect()
}
/// Computes the magnitude of the error at a located root via the Forney algorithm:
/// `locator_number * (omega(root) / derivative(root))`.
fn forney_magnitude(omega: GaloisPolynomial, derivative: GaloisPolynomial, locator_number: GaloisFieldNumber, root: GaloisFieldNumber) -> GaloisFieldNumber {
    locator_number * (omega.evaluate(root) / derivative.evaluate(root))
}
/// Attempts to correct errors in `block` (one QR data+EC codeword block) using Reed-Solomon
/// decoding. Returns `block` unchanged if its syndromes are already zero, or if more errors
/// are found than the locator's degree allows for, since the result can't be trusted then.
pub(crate) fn correct_block(block: &[u8], ec_len: EcLength) -> Vec<u8> {
    let n = block.len();
    let received = GaloisPolynomial(block.iter().rev().map(|&b| GaloisFieldNumber::from(b)).collect());

    let syndromes = compute_syndromes(&received, ec_len);
    if syndromes.is_zero() {
        return block.to_vec();
    }

    let locator = berlekamp_massey(syndromes.clone());
    let errors = chien_search(&locator, n);
    if errors.len() != locator.len() - 1 {
        // Too many errors to trust the result; give up rather than risk mis-correcting.
        return block.to_vec();
    }

    let omega = error_evaluator(syndromes, locator.clone());
    let derivative = locator.derivative();

    let mut corrected = block.to_vec();
    for LocatedError { position, locator_number } in errors {
        let root = !locator_number;
        let magnitude = forney_magnitude(omega.clone(), derivative.clone(), locator_number, root);
        corrected[n - 1 - position] ^= magnitude.0;
    }
    corrected
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf_add_is_xor() {
        assert_eq!(
            GaloisFieldNumber::from(5) + GaloisFieldNumber::from(3),
            GaloisFieldNumber::from(6)
        );
    }

    #[test]
    fn gf_mul_matches_known_values() {
        assert_eq!(
            GaloisFieldNumber::from(2) * GaloisFieldNumber::from(3),
            GaloisFieldNumber::from(6)
        );
        assert_eq!(
            GaloisFieldNumber::from(0) * GaloisFieldNumber::from(200),
            GaloisFieldNumber::from(0)
        );
    }

    #[test]
    fn gf_div_is_inverse_of_mul() {
        let a = GaloisFieldNumber::from(17);
        let b = GaloisFieldNumber::from(9);
        assert_eq!((a * b) / b, a);
    }

    #[test]
    fn poly_add_pads_the_shorter_operand_with_zero() {
        let a = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        let b = GaloisPolynomial(vec![4, 5].into_iter().map(GaloisFieldNumber::from).collect());
        let sum = a + b;
        let expected = GaloisPolynomial(vec![1 ^ 4, 2 ^ 5, 3].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(sum, expected);
    }

    #[test]
    fn poly_evaluate_at_zero_returns_constant_term() {
        let poly = GaloisPolynomial(vec![7, 1, 1].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(poly.evaluate(GaloisFieldNumber::from(0)), GaloisFieldNumber::from(7));
    }

    #[test]
    fn poly_evaluate_at_one_xors_all_coefficients() {
        let poly = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(poly.evaluate(GaloisFieldNumber::from(1)), GaloisFieldNumber::from(1 ^ 2 ^ 3));
    }

    #[test]
    fn poly_shift_by_zero_is_identity() {
        let poly = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(poly.clone().shift(0), poly);
    }

    #[test]
    fn poly_shift_prepends_zero_low_order_coefficients() {
        let poly = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        let expected = GaloisPolynomial(vec![0, 0, 1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(poly.shift(2), expected);
    }

    #[test]
    fn poly_evaluate_matches_horner_by_hand() {
        // P(x) = 3x^2 + 2x + 1, evaluated at x = 5.
        let poly = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        let x = GaloisFieldNumber::from(5);
        let expected = (GaloisFieldNumber::from(3) * x + GaloisFieldNumber::from(2)) * x + GaloisFieldNumber::from(1);
        assert_eq!(poly.evaluate(x), expected);
    }

    #[test]
    fn poly_mul_by_scalar_scales_every_coefficient() {
        let poly = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
        let scaled = poly * GaloisFieldNumber::from(2);
        let expected = GaloisPolynomial(
            vec![
                GaloisFieldNumber::from(1) * GaloisFieldNumber::from(2),
                GaloisFieldNumber::from(2) * GaloisFieldNumber::from(2),
                GaloisFieldNumber::from(3) * GaloisFieldNumber::from(2),
            ]
        );
        assert_eq!(scaled, expected);
    }

    #[test]
    fn poly_mul_computes_convolution() {
        // (1 + x) * (1 + x) = 1 + 2x + x^2, but GF(2) addition is XOR so the x term cancels.
        let a = GaloisPolynomial(vec![GaloisFieldNumber::from(1), GaloisFieldNumber::from(1)]);
        let product = a.clone() * a;
        let expected = GaloisPolynomial(vec![1, 0, 1].into_iter().map(GaloisFieldNumber::from).collect());
        assert_eq!(product, expected);
    }

    fn generator_as_poly(ec_len: EcLength) -> GaloisPolynomial {
        GaloisPolynomial(ec_len.generator_polynomial_bytes().iter().map(|&b| GaloisFieldNumber::from(b)).collect())
    }

    #[test]
    fn reed_solomon_remainder_of_empty_data_is_zero() {
        let ec_len = EcLength::try_from(7).unwrap();
        let remainder = reed_solomon_remainder(GaloisPolynomial(Vec::new()), ec_len);
        assert_eq!(remainder, GaloisPolynomial(vec![GaloisFieldNumber::default(); 7]));
    }

    #[test]
    fn reed_solomon_remainder_of_a_single_byte_is_the_generator_scaled_by_it() {
        // D(x) = d0 (a constant), so M(x) = d0 * x^t, and x^t mod G(x) is exactly
        // G(x)'s own lower t coefficients (since G is monic: x^t = G(x) - lower terms,
        // and subtraction is XOR). So the remainder is just d0 * generator.
        for &ec_len in EcLength::ALL.iter() {
            let d0 = GaloisFieldNumber::from(200);
            let remainder = reed_solomon_remainder(GaloisPolynomial(vec![d0]), ec_len);
            let expected = generator_as_poly(ec_len) * d0;
            assert_eq!(remainder, expected, "mismatch for ec_len = {}", ec_len.value());
        }
    }

    #[test]
    fn compute_syndromes_of_empty_received_is_zero() {
        let ec_len = EcLength::try_from(7).unwrap();
        let syndromes = compute_syndromes(&GaloisPolynomial(Vec::new()), ec_len);
        assert_eq!(syndromes, GaloisPolynomial(vec![GaloisFieldNumber::default(); 7]));
    }

    #[test]
    fn compute_syndromes_of_a_constant_polynomial_is_that_constant_everywhere() {
        // A constant polynomial evaluates to itself at every point, alpha^i included.
        let c = GaloisFieldNumber::from(42);
        let ec_len = EcLength::try_from(10).unwrap();
        let syndromes = compute_syndromes(&GaloisPolynomial(vec![c]), ec_len);
        assert_eq!(syndromes, GaloisPolynomial(vec![c; 10]));
    }

    #[test]
    fn compute_syndromes_length_matches_ec_len() {
        for &ec_len in EcLength::ALL.iter() {
            let received = GaloisPolynomial(vec![1, 2, 3].into_iter().map(GaloisFieldNumber::from).collect());
            let syndromes = compute_syndromes(&received, ec_len);
            assert_eq!(syndromes.len(), ec_len.value());
        }
    }

    #[test]
    fn berlekamp_massey_of_all_zero_syndromes_is_the_trivial_locator() {
        // No errors means every syndrome is zero, and the algorithm never finds a
        // nonzero discrepancy to react to, so it should leave the locator at 1.
        let syndromes = GaloisPolynomial(vec![GaloisFieldNumber::default(); 6]);
        let locator = berlekamp_massey(syndromes);
        assert_eq!(locator, GaloisPolynomial(vec![GaloisFieldNumber::from(1)]));
    }

    #[test]
    fn berlekamp_massey_finds_the_locator_for_a_single_known_error_position() {
        // A single error at locator value X produces the geometric syndrome sequence
        // S_i = X^i = alpha^i (taking X = alpha^1), whose minimal polynomial is 1 + X*x.
        // Hand-traced against the implementation for i = 0..4 to confirm this by hand,
        // not just by re-deriving the same formula the code computes.
        let x = GaloisFieldNumber::from(EXP_TABLE[1]);
        let syndromes = GaloisPolynomial((0..4).map(|i| GaloisFieldNumber::from(EXP_TABLE[i])).collect());
        let locator = berlekamp_massey(syndromes);
        assert_eq!(locator, GaloisPolynomial(vec![GaloisFieldNumber::from(1), x]));
    }

    #[test]
    fn chien_search_with_trivial_locator_finds_no_errors() {
        // Lambda(x) = 1 (a constant) evaluates to 1 everywhere, so it has no roots at all.
        let locator = GaloisPolynomial(vec![GaloisFieldNumber::from(1)]);
        assert!(chien_search(&locator, 10).is_empty());
    }

    #[test]
    fn chien_search_finds_the_single_known_error_position() {
        // Lambda(x) = 1 + X*x has its one root at x = X^-1, i.e. at alpha^-j0 where
        // X = alpha^j0. So the search should match only at j = j0, reporting locator_number = X.
        let j0 = 1;
        let x = GaloisFieldNumber::from(EXP_TABLE[j0]);
        let locator = GaloisPolynomial(vec![GaloisFieldNumber::from(1), x]);

        let errors = chien_search(&locator, 4);

        assert_eq!(errors, vec![LocatedError { position: j0, locator_number: x }]);
    }

    // Builds a valid systematic codeword (in the natural, MSB-first byte order a real QR
    // code would be read in) for the given data, using the already-verified encoder.
    fn encode_block(data_bytes: &[u8], ec_len: EcLength) -> Vec<u8> {
        let data = GaloisPolynomial(data_bytes.iter().map(|&b| GaloisFieldNumber::from(b)).collect());
        let remainder = reed_solomon_remainder(data.clone(), ec_len);
        let codeword = data.shift(ec_len.value()) + remainder;
        codeword.0.iter().rev().map(|c| c.0).collect()
    }

    #[test]
    fn correct_block_with_no_errors_returns_it_unchanged() {
        let ec_len = EcLength::try_from(7).unwrap();
        let block = encode_block(&[1, 2, 3], ec_len);
        assert_eq!(correct_block(&block, ec_len), block);
    }

    #[test]
    fn correct_block_recovers_from_a_single_byte_error() {
        let ec_len = EcLength::try_from(7).unwrap();
        let original_block = encode_block(&[10, 20, 30, 40], ec_len);

        let mut corrupted_block = original_block.clone();
        corrupted_block[2] ^= 0xFF;

        assert_eq!(correct_block(&corrupted_block, ec_len), original_block);
    }

    #[test]
    fn correct_block_recovers_from_two_byte_errors() {
        // ec_len = 7 can correct up to floor(7/2) = 3 errors.
        let ec_len = EcLength::try_from(7).unwrap();
        let original_block = encode_block(&[5, 15, 25, 35, 45], ec_len);

        let mut corrupted_block = original_block.clone();
        corrupted_block[0] ^= 0x55;
        corrupted_block[4] ^= 0xAA;

        assert_eq!(correct_block(&corrupted_block, ec_len), original_block);
    }
}