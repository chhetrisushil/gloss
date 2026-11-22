pub const SCALE: i64 = 10_000;

/// What a number literal beyond the range says.
pub const OUT_OF_RANGE: &str =
    "This number is outside the range Gloss can hold, about 922 trillion either way";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number(pub i64);

/// A count of decimal places, 0 to 4, so showing a number can never be asked for more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Places(u8);

impl Places {
    pub fn new(places: u8) -> Option<Places> {
        (places <= 4).then_some(Places(places))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    Malformed,
    TooManyPlaces,
    OutOfRange,
}

/// Why arithmetic on numbers failed at run time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumFail {
    DivideByZero,
    OutOfRange,
}

fn in_range(v: i128) -> Result<Number, NumFail> {
    i64::try_from(v)
        .map(Number)
        .map_err(|_| NumFail::OutOfRange)
}

/// `numerator / denominator` rounded half away from zero; the denominator is not zero.
fn divide_rounded(numerator: i128, denominator: i128) -> i128 {
    let (n, d) = (numerator.abs(), denominator.abs());
    let sign = numerator.signum() * denominator.signum();
    let (q, r) = (n / d, n % d);
    // Round up when the remainder is at least half the divisor, without doubling n.
    sign * (q + i128::from(r >= d - r))
}

impl Number {
    pub fn parse_literal(s: &str) -> Result<Number, NumberError> {
        parse_digits(s, false)
    }

    pub fn parse_text(s: &str) -> Result<Number, NumberError> {
        let t = s.trim_matches(' ');
        match t.strip_prefix('-') {
            Some(rest) => parse_digits(rest, true),
            None => parse_digits(t, false),
        }
    }

    pub fn checked_add(self, o: Number) -> Result<Number, NumFail> {
        in_range(i128::from(self.0) + i128::from(o.0))
    }

    pub fn checked_sub(self, o: Number) -> Result<Number, NumFail> {
        in_range(i128::from(self.0) - i128::from(o.0))
    }

    pub fn checked_neg(self) -> Result<Number, NumFail> {
        in_range(-i128::from(self.0))
    }

    /// `self * o`, rounded half away from zero to four places.
    pub fn mul_round(self, o: Number) -> Result<Number, NumFail> {
        in_range(divide_rounded(
            i128::from(self.0) * i128::from(o.0),
            i128::from(SCALE),
        ))
    }

    /// `self / o`, rounded half away from zero to four places.
    pub fn div_round(self, o: Number) -> Result<Number, NumFail> {
        if o.0 == 0 {
            return Err(NumFail::DivideByZero);
        }
        in_range(divide_rounded(
            i128::from(self.0) * i128::from(SCALE),
            i128::from(o.0),
        ))
    }

    /// The exact remainder; its sign follows `self`. Fallible, so not `std::ops::Rem`.
    #[allow(clippy::should_implement_trait)]
    pub fn rem(self, o: Number) -> Result<Number, NumFail> {
        if o.0 == 0 {
            return Err(NumFail::DivideByZero);
        }
        in_range(i128::from(self.0) % i128::from(o.0))
    }

    /// Drops the fraction, toward zero.
    pub fn whole(self) -> Number {
        Number(self.0 / SCALE * SCALE)
    }

    pub fn show(self) -> String {
        let v = self.0 as i128;
        let (whole, frac) = (v.abs() / 10_000, v.abs() % 10_000);
        let mut s = if v < 0 {
            String::from("-")
        } else {
            String::new()
        };
        s.push_str(&whole.to_string());
        if frac != 0 {
            s.push('.');
            s.push_str(format!("{frac:04}").trim_end_matches('0'));
        }
        s
    }

    pub fn with_places(self, places: Places) -> String {
        let places = places.0;
        let v = self.0 as i128;
        let unit = 10i128.pow(4 - places as u32);
        let rounded = (v.abs() + unit / 2) / unit;
        let p = 10i128.pow(places as u32);
        let mut s = if v < 0 && rounded != 0 {
            String::from("-")
        } else {
            String::new()
        };
        s.push_str(&(rounded / p).to_string());
        if places > 0 {
            s.push('.');
            s.push_str(&format!("{:0w$}", rounded % p, w = places as usize));
        }
        s
    }
}

fn parse_digits(s: &str, negative: bool) -> Result<Number, NumberError> {
    use NumberError::*;
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) {
        return Err(Malformed);
    }
    let frac = match frac {
        None => "",
        Some(f) if !digits(f) => return Err(Malformed),
        Some(f) if f.len() > 4 => return Err(TooManyPlaces),
        Some(f) => f,
    };
    let mut v: i128 = 0;
    for b in whole.bytes() {
        v = v * 10 + i128::from(b - b'0');
        if v > i128::from(i64::MAX) {
            return Err(OutOfRange);
        }
    }
    let mut f: i128 = frac.bytes().fold(0, |a, b| a * 10 + i128::from(b - b'0'));
    for _ in frac.len()..4 {
        f *= 10;
    }
    let total = v * 10_000 + f;
    i64::try_from(if negative { -total } else { total })
        .map(Number)
        .map_err(|_| OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use NumberError::*;
    fn n(v: i64) -> Number {
        Number(v)
    }
    fn pl(p: u8) -> Places {
        Places::new(p).unwrap()
    }

    #[test]
    fn literals() {
        assert_eq!(Number::parse_literal("120.50"), Ok(n(1_205_000)));
        assert_eq!(Number::parse_literal("42"), Ok(n(420_000)));
        assert_eq!(Number::parse_literal("007"), Ok(n(70_000)));
        assert_eq!(Number::parse_literal("0.12345"), Err(TooManyPlaces));
        assert_eq!(Number::parse_literal("5."), Err(Malformed));
        assert_eq!(Number::parse_literal(".5"), Err(Malformed));
        assert_eq!(Number::parse_literal("-5"), Err(Malformed));
        assert_eq!(
            Number::parse_literal("922337203685477.5807"),
            Ok(n(i64::MAX))
        );
        assert_eq!(
            Number::parse_literal("922337203685477.5808"),
            Err(OutOfRange)
        );
        assert_eq!(
            Number::parse_literal("99999999999999999999999"),
            Err(OutOfRange)
        );
    }

    #[test]
    fn text_follows_as_number_rules() {
        assert_eq!(Number::parse_text(" -7 "), Ok(n(-70_000)));
        assert_eq!(Number::parse_text("0.5"), Ok(n(5_000)));
        assert_eq!(Number::parse_text("-922337203685477.5808"), Ok(n(i64::MIN)));
        for bad in ["+3", "1,200", "1e3", ".5", "", "-", "- 3", "12a", "1.2.3"] {
            assert_eq!(Number::parse_text(bad), Err(Malformed), "{bad:?}");
        }
        assert_eq!(Number::parse_text("0.12345"), Err(TooManyPlaces));
    }

    #[test]
    fn show_drops_trailing_zeros() {
        assert_eq!(n(1_205_000).show(), "120.5");
        assert_eq!(n(1_200_000).show(), "120");
        assert_eq!(n(-35_000).show(), "-3.5");
        assert_eq!(n(3_333).show(), "0.3333");
        assert_eq!(n(0).show(), "0");
        assert_eq!(n(i64::MIN).show(), "-922337203685477.5808");
    }

    #[test]
    fn with_places_rounds_half_away_from_zero() {
        assert_eq!(n(1_205_000).with_places(pl(2)), "120.50");
        assert_eq!(n(1_205_000).with_places(pl(0)), "121");
        assert_eq!(n(-1_205_000).with_places(pl(0)), "-121");
        assert_eq!(n(3_333).with_places(pl(2)), "0.33");
        assert_eq!(n(-4_000).with_places(pl(0)), "0");
        assert_eq!(n(0).with_places(pl(4)), "0.0000");
        assert_eq!(n(i64::MAX).with_places(pl(0)), "922337203685478");
    }

    #[test]
    fn with_places_at_the_edges() {
        assert_eq!(n(i64::MIN).with_places(pl(0)), "-922337203685478");
        assert_eq!(n(i64::MIN).with_places(pl(4)), "-922337203685477.5808");
        assert_eq!(n(-500).with_places(pl(1)), "-0.1");
        assert_eq!(n(-499).with_places(pl(1)), "0.0");
    }

    #[test]
    fn places_stop_at_four() {
        assert!(Places::new(4).is_some());
        assert_eq!(Places::new(5), None);
    }

    #[test]
    fn negative_zero_text_is_zero() {
        assert_eq!(Number::parse_text("-0"), Ok(n(0)));
        assert_eq!(Number::parse_text("-0.0000"), Ok(n(0)));
    }

    const MAX: Number = Number(i64::MAX);

    #[test]
    fn add_sub_neg_are_exact_or_out_of_range() {
        assert_eq!(n(1000).checked_add(n(2000)), Ok(n(3000)));
        assert_eq!(MAX.checked_add(n(1)), Err(NumFail::OutOfRange));
        assert_eq!(n(i64::MIN).checked_sub(n(1)), Err(NumFail::OutOfRange));
        assert_eq!(n(5).checked_sub(n(8)), Ok(n(-3)));
        assert_eq!(n(i64::MIN).checked_neg(), Err(NumFail::OutOfRange));
        assert_eq!(n(7).checked_neg(), Ok(n(-7)));
    }

    #[test]
    fn division_rounds_half_away_from_zero() {
        assert_eq!(n(70_000).div_round(n(20_000)), Ok(n(35_000)));
        assert_eq!(n(10_000).div_round(n(30_000)), Ok(n(3333)));
        assert_eq!(n(20_000).div_round(n(30_000)), Ok(n(6667)));
        assert_eq!(n(-20_000).div_round(n(30_000)), Ok(n(-6667)));
        assert_eq!(n(20_000).div_round(n(-30_000)), Ok(n(-6667)));
        assert_eq!(n(i64::MIN).div_round(n(-10_000)), Err(NumFail::OutOfRange));
        assert_eq!(n(5).div_round(n(0)), Err(NumFail::DivideByZero));
    }

    #[test]
    fn multiplication_rounds_half_away_from_zero() {
        assert_eq!(n(1).mul_round(n(5000)), Ok(n(1)));
        assert_eq!(n(-1).mul_round(n(5000)), Ok(n(-1)));
        assert_eq!(n(1).mul_round(n(4999)), Ok(n(0)));
        assert_eq!(n(3333).mul_round(n(30_000)), Ok(n(9999)));
        assert_eq!(MAX.mul_round(n(20_000)), Err(NumFail::OutOfRange));
    }

    #[test]
    fn remainder_and_whole() {
        assert_eq!(n(70_000).rem(n(20_000)), Ok(n(10_000)));
        assert_eq!(n(-70_000).rem(n(20_000)), Ok(n(-10_000)));
        assert_eq!(n(75_000).rem(n(20_000)), Ok(n(15_000)));
        assert_eq!(n(i64::MIN).rem(n(-1)), Ok(n(0)));
        assert_eq!(n(5).rem(n(0)), Err(NumFail::DivideByZero));
        assert_eq!(n(35_000).whole(), n(30_000));
        assert_eq!(n(-35_000).whole(), n(-30_000));
    }

    #[test]
    fn mul_round_matches_an_independent_formula() {
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as i64
        };
        for i in 0..4000 {
            let (a, b) = if i % 2 == 0 {
                (next(), next())
            } else {
                (
                    next().rem_euclid(200_000_001) - 100_000_000,
                    next().rem_euclid(200_000_001) - 100_000_000,
                )
            };
            let p = i128::from(a) * i128::from(b);
            let expected = i64::try_from((p + p.signum() * 5000) / 10_000)
                .map(Number)
                .map_err(|_| NumFail::OutOfRange);
            assert_eq!(n(a).mul_round(n(b)), expected, "{a} * {b}");
        }
    }

    #[test]
    fn extremes_are_out_of_range_not_a_panic() {
        let min = n(i64::MIN);
        let out = Err(NumFail::OutOfRange);
        assert_eq!(min.mul_round(min), out);
        assert_eq!(min.mul_round(MAX), out);
        assert_eq!(MAX.mul_round(min), out);
        assert_eq!(MAX.mul_round(MAX), out);
        assert_eq!(MAX.mul_round(n(10_000)), Ok(MAX));
        assert_eq!(min.mul_round(n(10_000)), Ok(min));
        assert_eq!(min.div_round(n(1)), out);
        assert_eq!(MAX.div_round(n(1)), out);
        assert_eq!(n(0).div_round(n(1)), Ok(n(0)));
        assert_eq!(min.div_round(min), Ok(n(10_000)));
        assert_eq!(MAX.div_round(MAX), Ok(n(10_000)));
    }
}
