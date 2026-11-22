pub const SCALE: i64 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberError {
    Malformed,
    TooManyPlaces,
    OutOfRange,
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

    pub fn with_places(self, places: u8) -> String {
        assert!(places <= 4, "places is checked by the parser");
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
        assert_eq!(n(1_205_000).with_places(2), "120.50");
        assert_eq!(n(1_205_000).with_places(0), "121");
        assert_eq!(n(-1_205_000).with_places(0), "-121");
        assert_eq!(n(3_333).with_places(2), "0.33");
        assert_eq!(n(-4_000).with_places(0), "0");
        assert_eq!(n(0).with_places(4), "0.0000");
        assert_eq!(n(i64::MAX).with_places(0), "922337203685478");
    }

    #[test]
    fn with_places_at_the_edges() {
        assert_eq!(n(i64::MIN).with_places(0), "-922337203685478");
        assert_eq!(n(i64::MIN).with_places(4), "-922337203685477.5808");
        assert_eq!(n(-500).with_places(1), "-0.1");
        assert_eq!(n(-499).with_places(1), "0.0");
    }

    #[test]
    fn negative_zero_text_is_zero() {
        assert_eq!(Number::parse_text("-0"), Ok(n(0)));
        assert_eq!(Number::parse_text("-0.0000"), Ok(n(0)));
    }
}
