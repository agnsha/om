//! The prelude: Om source compiled into every program. Most of it is in
//! `prelude.om`; the impls repeated for each number type are generated here.

use std::fmt::Write;

const SIGNED: [&str; 5] = ["i8", "i16", "i32", "i64", "isize"];
const UNSIGNED: [&str; 5] = ["u8", "u16", "u32", "u64", "usize"];
const FLOATS: [&str; 2] = ["f32", "f64"];

/// The complete prelude source.
pub fn source() -> String {
    let mut s = String::from(include_str!("prelude.om"));
    for t in SIGNED.iter().chain(&UNSIGNED) {
        numeric_traits(&mut s, t, "0");
        int_methods(&mut s, t, SIGNED.contains(t));
    }
    for t in FLOATS {
        numeric_traits(&mut s, t, "0.0");
        float_methods(&mut s, t);
    }
    s.push_str(CHAR_AND_BOOL);
    s
}

/// `Eq`, `Ord`, arithmetic traits and `Default` for a number type.
fn numeric_traits(s: &mut String, t: &str, zero: &str) {
    let _ = writeln!(
        s,
        "
impl Eq for {t} {{ fn eq(&self, other: &{t}) -> bool {{ self == other }} }}
impl Ord for {t} {{
    fn cmp(&self, other: &{t}) -> Ordering {{
        if self < other {{ Ordering::Less }} else if self > other {{ Ordering::Greater }} else {{ Ordering::Equal }}
    }}
}}
impl Add for {t} {{ fn add(self, other: {t}) -> {t} {{ self + other }} }}
impl Sub for {t} {{ fn sub(self, other: {t}) -> {t} {{ self - other }} }}
impl Mul for {t} {{ fn mul(self, other: {t}) -> {t} {{ self * other }} }}
impl Div for {t} {{ fn div(self, other: {t}) -> {t} {{ self / other }} }}
impl Default for {t} {{ fn default() -> {t} {{ {zero} }} }}"
    );
    if !UNSIGNED.contains(&t) {
        let _ = writeln!(s, "impl Neg for {t} {{ fn neg(self) -> {t} {{ -self }} }}");
    }
    let _ = writeln!(s, "impl Rem for {t} {{ fn rem(self, other: {t}) -> {t} {{ self % other }} }}");
}

fn int_methods(s: &mut String, t: &str, signed: bool) {
    let _ = writeln!(
        s,
        "
impl {t} {{
    fn pow(self, exp: u32) -> {t} {{
        let mut result: {t} = 1
        let mut base = self
        let mut e = exp
        while e > 0 {{
            if e & 1 == 1 {{ result = result * base }}
            e = e >> 1
            // Squaring after the last bit could overflow even when the result fits.
            if e > 0 {{ base = base * base }}
        }}
        result
    }}
    fn count_ones(self) -> u32 {{ intrinsic::popcnt(self) }}
    fn count_zeros(self) -> u32 {{ intrinsic::popcnt(!self) }}
    fn leading_zeros(self) -> u32 {{ intrinsic::clz(self) }}
    fn trailing_zeros(self) -> u32 {{ intrinsic::ctz(self) }}
    fn wrapping_add(self, other: {t}) -> {t} {{ intrinsic::wrapping_add(self, other) }}
    fn wrapping_sub(self, other: {t}) -> {t} {{ intrinsic::wrapping_sub(self, other) }}
    fn wrapping_mul(self, other: {t}) -> {t} {{ intrinsic::wrapping_mul(self, other) }}
    fn checked_add(self, other: {t}) -> Option[{t}] {{
        let r = intrinsic::wrapping_add(self, other)
        if (other >= 0) == (r >= self) {{ Some(r) }} else {{ None }}
    }}
    fn checked_sub(self, other: {t}) -> Option[{t}] {{
        let r = intrinsic::wrapping_sub(self, other)
        if (other >= 0) == (r <= self) {{ Some(r) }} else {{ None }}
    }}
    fn checked_mul(self, other: {t}) -> Option[{t}] {{
        if self == 0 || other == 0 {{ return Some(0) }}
        {mul_guard}
        let r = intrinsic::wrapping_mul(self, other)
        if r / other == self {{ Some(r) }} else {{ None }}
    }}
    fn checked_div(self, other: {t}) -> Option[{t}] {{
        if other == 0 {{ return None }}
        {div_guard}
        Some(self / other)
    }}
    fn saturating_add(self, other: {t}) -> {t} {{
        match self.checked_add(other) {{ Some(r) => r, None => if other > 0 {{ {t}::MAX }} else {{ {t}::MIN }} }}
    }}
    fn saturating_sub(self, other: {t}) -> {t} {{
        match self.checked_sub(other) {{ Some(r) => r, None => if other > 0 {{ {t}::MIN }} else {{ {t}::MAX }} }}
    }}
    fn is_power_of_two(self) -> bool {{ self > 0 && (self & (self - 1)) == 0 }}",
        mul_guard = if signed {
            format!("if (self == -1 && other == {t}::MIN) || (other == -1 && self == {t}::MIN) {{ return None }}")
        } else {
            String::new()
        },
        div_guard =
            if signed { format!("if self == {t}::MIN && other == -1 {{ return None }}") } else { String::new() },
    );
    if signed {
        let _ = writeln!(
            s,
            "    fn abs(self) -> {t} {{ if self < 0 {{ -self }} else {{ self }} }}
    fn signum(self) -> {t} {{ if self > 0 {{ 1 }} else if self < 0 {{ -1 }} else {{ 0 }} }}
    fn is_positive(self) -> bool {{ self > 0 }}
    fn is_negative(self) -> bool {{ self < 0 }}
    fn rem_euclid(self, other: {t}) -> {t} {{
        let r = self % other
        if r < 0 {{ if other < 0 {{ r - other }} else {{ r + other }} }} else {{ r }}
    }}"
        );
    }
    s.push_str("}\n");
}

fn float_methods(s: &mut String, t: &str) {
    // Math functions without a machine instruction go through C's libm in f64.
    let mut libm = String::new();
    for f in ["sin", "cos", "tan", "asin", "acos", "atan", "exp", "ln", "log10", "log2", "cbrt"] {
        let _ = writeln!(libm, "    fn {f}(self) -> {t} {{ intrinsic::libm_{f}(self as f64) as {t} }}");
    }
    for f in ["powf", "atan2", "hypot"] {
        let _ = writeln!(
            libm,
            "    fn {f}(self, other: {t}) -> {t} {{ intrinsic::libm_{f}(self as f64, other as f64) as {t} }}"
        );
    }
    let _ = writeln!(
        s,
        "
impl {t} {{
    fn sqrt(self) -> {t} {{ intrinsic::sqrt(self) }}
    fn floor(self) -> {t} {{ intrinsic::floor(self) }}
    fn ceil(self) -> {t} {{ intrinsic::ceil(self) }}
    fn trunc(self) -> {t} {{ intrinsic::trunc(self) }}
    fn abs(self) -> {t} {{ intrinsic::fabs(self) }}
    fn round(self) -> {t} {{
        // Halfway cases round away from zero, as in Rust.
        let r = self.trunc()
        if (self - r).abs() >= 0.5 {{ if self < 0.0 {{ r - 1.0 }} else {{ r + 1.0 }} }} else {{ r }}
    }}
    fn fract(self) -> {t} {{ self - self.trunc() }}
    fn signum(self) -> {t} {{ if self.is_nan() {{ self }} else if self < 0.0 {{ -1.0 }} else {{ 1.0 }} }}
    fn is_nan(self) -> bool {{ self != self }}
    fn is_infinite(self) -> bool {{ self == {t}::INFINITY || self == -{t}::INFINITY }}
    fn is_finite(self) -> bool {{ !self.is_nan() && !self.is_infinite() }}
    fn powi(self, n: i32) -> {t} {{
        let mut result: {t} = 1.0
        let mut base = self
        let mut e = if n < 0 {{ -(n as i64) }} else {{ n as i64 }}
        while e > 0 {{
            if e % 2 == 1 {{ result = result * base }}
            base = base * base
            e = e / 2
        }}
        if n < 0 {{ 1.0 / result }} else {{ result }}
    }}
    fn to_degrees(self) -> {t} {{ self * 57.29577951308232 }}
    fn to_radians(self) -> {t} {{ self * 0.017453292519943295 }}
{libm}}}"
    );
}

const CHAR_AND_BOOL: &str = r#"
impl Eq for bool { fn eq(&self, other: &bool) -> bool { self == other } }
impl Default for bool { fn default() -> bool { false } }

impl Eq for char { fn eq(&self, other: &char) -> bool { self == other } }
impl Ord for char {
    fn cmp(&self, other: &char) -> Ordering {
        if self < other { Ordering::Less } else if self > other { Ordering::Greater } else { Ordering::Equal }
    }
}
impl Default for char { fn default() -> char { '\0' } }

impl char {
    fn from_u32(n: u32) -> Option[char] {
        if n > 0x10FFFF || (n >= 0xD800 && n <= 0xDFFF) { None } else { Some(intrinsic::u32_to_char(n)) }
    }
    fn from_digit(n: u32, radix: u32) -> Option[char] {
        if n >= radix || radix > 36 { None }
        else if n < 10 { Some(((n + 48) as u8) as char) }
        else { Some(((n - 10 + 97) as u8) as char) }
    }
    fn to_digit(self, radix: u32) -> Option[u32] {
        let c = self as u32
        let d = if c >= 48 && c <= 57 { c - 48 }
            else if c >= 97 && c <= 122 { c - 97 + 10 }
            else if c >= 65 && c <= 90 { c - 65 + 10 }
            else { 99 }
        if d < radix { Some(d) } else { None }
    }
    fn is_digit(self, radix: u32) -> bool { self.to_digit(radix).is_some() }
    fn is_ascii(self) -> bool { (self as u32) < 128 }
    fn is_ascii_digit(self) -> bool { match self { '0'..='9' => true, _ => false } }
    fn is_ascii_hexdigit(self) -> bool { match self { '0'..='9' | 'a'..='f' | 'A'..='F' => true, _ => false } }
    fn is_ascii_lowercase(self) -> bool { match self { 'a'..='z' => true, _ => false } }
    fn is_ascii_uppercase(self) -> bool { match self { 'A'..='Z' => true, _ => false } }
    fn is_ascii_alphabetic(self) -> bool { self.is_ascii_lowercase() || self.is_ascii_uppercase() }
    fn is_ascii_alphanumeric(self) -> bool { self.is_ascii_alphabetic() || self.is_ascii_digit() }
    fn is_ascii_whitespace(self) -> bool { match self { ' ' | '\t' | '\n' | '\r' | '\u{c}' => true, _ => false } }
    fn is_ascii_punctuation(self) -> bool {
        match self { '!'..='/' | ':'..='@' | '['..='`' | '{'..='~' => true, _ => false }
    }
    fn is_ascii_control(self) -> bool { match self { '\0'..='\u{1f}' | '\u{7f}' => true, _ => false } }
    fn is_whitespace(self) -> bool {
        match self {
            '\t'..='\r' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
                | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' => true,
            _ => false,
        }
    }
    fn to_ascii_uppercase(self) -> char {
        if self.is_ascii_lowercase() { intrinsic::u32_to_char(self as u32 - 32) } else { self }
    }
    fn to_ascii_lowercase(self) -> char {
        if self.is_ascii_uppercase() { intrinsic::u32_to_char(self as u32 + 32) } else { self }
    }
    fn len_utf8(self) -> usize {
        let c = self as u32
        if c < 0x80 { 1 } else if c < 0x800 { 2 } else if c < 0x10000 { 3 } else { 4 }
    }
}
"#;
