//! Shared parser for integer byte quantities with IEC binary units.

/// A byte quantity parsed by [`parse_iec_quantity`]. `has_unit` lets callers
/// require or forbid a unit.
pub(crate) struct IecQuantity {
    pub(crate) bytes: u64,
    pub(crate) has_unit: bool,
}

const UNITS: [(&str, u64); 4] = [
    ("Ki", 1 << 10),
    ("Mi", 1 << 20),
    ("Gi", 1 << 30),
    ("Ti", 1 << 40),
];

/// Parses an unsigned integer with an optional IEC binary unit (Ki, Mi, Gi, Ti).
/// Signs, fractions, exponents, whitespace and leading zeros are rejected; `0`
/// itself is allowed. Returns `None` if the input is malformed or overflows u64.
pub(crate) fn parse_iec_quantity(input: &str) -> Option<IecQuantity> {
    let (digits, multiplier) = UNITS
        .into_iter()
        .find_map(|(unit, multiplier)| input.strip_suffix(unit).map(|d| (d, Some(multiplier))))
        .unwrap_or((input, None));
    // u64::from_str accepts a leading '+', so check the digits first.
    let well_formed = !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'));
    if !well_formed {
        return None;
    }
    let bytes = digits
        .parse::<u64>()
        .ok()?
        .checked_mul(multiplier.unwrap_or(1))?;
    Some(IecQuantity {
        bytes,
        has_unit: multiplier.is_some(),
    })
}

#[cfg(test)]
mod test_parse_iec_quantity {
    use super::parse_iec_quantity;

    #[test]
    fn valid() {
        for (input, bytes, has_unit) in [
            ("0", 0, false),
            ("1", 1, false),
            ("1048576", 1 << 20, false),
            ("18446744073709551615", u64::MAX, false),
            ("0Ki", 0, true),
            ("1Ki", 1 << 10, true),
            ("512Mi", 512 << 20, true),
            ("2Gi", 2 << 30, true),
            ("1Ti", 1 << 40, true),
            ("16777215Ti", ((1 << 24) - 1) << 40, true),
        ] {
            let quantity = parse_iec_quantity(input).unwrap();
            assert_eq!(quantity.bytes, bytes, "{input}");
            assert_eq!(quantity.has_unit, has_unit, "{input}");
        }
    }

    #[test]
    fn invalid() {
        for input in [
            "",
            "Mi",
            "00",
            "01",
            "01Gi",
            "00Mi",
            "+1",
            "-1",
            "1.5Gi",
            "1e9",
            " 1Gi",
            "1Gi ",
            "1 Gi",
            "1mi",
            "1M",
            "1Pi",
            "1Gib",
            "1GiGi",
            "１Gi",
            "18446744073709551616",
            "16777216Ti",
        ] {
            assert!(parse_iec_quantity(input).is_none(), "{input:?}");
        }
    }
}
