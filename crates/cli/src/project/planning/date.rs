use super::super::fault;
use concord_core::Result;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::project) struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    pub fn parse(value: &str) -> Result<Self> {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || !value.is_ascii() {
            return Err(fault(
                "date",
                "use an explicit YYYY-MM-DD local calendar date, without a timestamp or implicit UTC conversion",
            ));
        }
        if bytes[4] != b'-' || bytes[7] != b'-' {
            return Err(fault("date", "native cycle dates require YYYY-MM-DD"));
        }
        if bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| !matches!(index, 4 | 7) && !byte.is_ascii_digit())
        {
            return Err(fault("date", "native cycle dates require decimal digits"));
        }
        let year = value[..4].parse::<u16>().ok();
        let month = value[5..7].parse::<u8>().ok();
        let day = value[8..].parse::<u8>().ok();
        let (Some(year), Some(month), Some(day)) = (year, month, day) else {
            return Err(fault("date", "native cycle date contains invalid digits"));
        };
        if year == 0 || month == 0 || month > 12 {
            return Err(fault(
                "date",
                "native cycle date is outside the Gregorian calendar",
            ));
        }
        if day == 0 || day > days(year, month) {
            return Err(fault("date", "native cycle day does not exist"));
        }
        Ok(Self { year, month, day })
    }

    pub fn end(mut self, duration: u16) -> Result<Self> {
        if duration == 0 || duration > 366 {
            return Err(fault(
                "duration",
                "cycle duration requires 1..366 calendar days",
            ));
        }
        for _ in 0..duration {
            if self.day < days(self.year, self.month) {
                self.day += 1;
                continue;
            }
            self.day = 1;
            if self.month < 12 {
                self.month += 1;
                continue;
            }
            self.month = 1;
            if self.year == 9999 {
                return Err(fault("date", "cycle end exceeds the supported calendar"));
            }
            self.year += 1;
        }
        Ok(self)
    }

    pub fn day(self) -> i64 {
        let years = (1..self.year)
            .map(|year| if days(year, 2) == 29 { 366i64 } else { 365i64 })
            .sum::<i64>();
        let months = (1..self.month)
            .map(|month| i64::from(days(self.year, month)))
            .sum::<i64>();
        years + months + i64::from(self.day) - 1 - 719162
    }

    pub fn text(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

fn days(year: u16, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::Date;

    #[test]
    fn calendar() {
        for (start, duration, end) in [
            ("2026-10-21", 7, "2026-10-28"),
            ("2024-02-28", 2, "2024-03-01"),
            ("2000-02-28", 2, "2000-03-01"),
            ("2100-02-28", 1, "2100-03-01"),
            ("2026-12-29", 7, "2027-01-05"),
        ] {
            assert_eq!(
                Date::parse(start).unwrap().end(duration).unwrap().text(),
                end
            );
        }
    }

    #[test]
    fn boundaries() {
        for value in [
            "2026-10-21T00:30:00+08:00",
            "2026-10-20T16:30:00Z",
            "2026-02-29",
            "1900-02-29",
            "0000-01-01",
            "2026-1-01",
            "２０２６-10-21",
            "+026-10-21",
        ] {
            assert!(Date::parse(value).is_err(), "{value}");
        }
        assert!(Date::parse("9999-12-31").unwrap().end(1).is_err());
        assert!(Date::parse("2026-10-21").unwrap().end(0).is_err());
    }
}
