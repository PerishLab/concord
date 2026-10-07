use concord_core::Result;

pub(super) fn timestamp(value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    let separators = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if bytes.len() != 20
        || separators
            .iter()
            .any(|(index, byte)| bytes.get(*index) != Some(byte))
    {
        return Err(super::fault(
            "reply",
            "expected a canonical UTC provider timestamp",
        ));
    }
    for index in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18] {
        if !bytes[index].is_ascii_digit() {
            return Err(super::fault("reply", "invalid provider timestamp digits"));
        }
    }
    let number = |start: usize, end: usize| value[start..end].parse::<u32>().unwrap();
    let year = number(0, 4);
    let month = number(5, 7);
    let day = number(8, 10);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    let ranges = [
        (year, 1, 9999),
        (month, 1, 12),
        (day, 1, days),
        (number(11, 13), 0, 23),
        (number(14, 16), 0, 59),
        (number(17, 19), 0, 59),
    ];
    if ranges
        .iter()
        .any(|(number, min, max)| !(min..=max).contains(&number))
    {
        return Err(super::fault(
            "reply",
            "provider timestamp is outside calendar bounds",
        ));
    }
    Ok(())
}
