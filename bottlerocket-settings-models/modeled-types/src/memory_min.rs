//! Strict byte quantities for kube-reserved service reclaim protection.
use crate::error;
use bottlerocket_string_impls_for::string_impls_for;
use serde::Deserialize;
use snafu::ensure;

/// An unsigned integer byte count, optionally suffixed with Ki, Mi, or Gi.
/// Zero disables protection. Each service is limited to half a u64 so the
/// two-service parent sum is always representable; this is not a sizing policy.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KubeReservedMemoryMinValue {
    inner: String,
}

impl KubeReservedMemoryMinValue {
    pub const MAX_BYTES: u64 = u64::MAX / 2;

    pub fn as_bytes(&self) -> u64 {
        // All constructors and deserialization validate this immutable string.
        Self::parse(&self.inner).expect("validated memory.min quantity")
    }

    fn parse(input: &str) -> Option<u64> {
        let (digits, multiplier) = if let Some(n) = input.strip_suffix("Ki") {
            (n, 1_u64 << 10)
        } else if let Some(n) = input.strip_suffix("Mi") {
            (n, 1_u64 << 20)
        } else if let Some(n) = input.strip_suffix("Gi") {
            (n, 1_u64 << 30)
        } else {
            (input, 1)
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let bytes = digits.parse::<u64>().ok()?.checked_mul(multiplier)?;
        (bytes <= Self::MAX_BYTES).then_some(bytes)
    }
}

impl TryFrom<&str> for KubeReservedMemoryMinValue {
    type Error = error::Error;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        ensure!(
            Self::parse(input).is_some(),
            error::InvalidKubeReservedMemoryMinSnafu { input }
        );
        Ok(Self {
            inner: input.to_owned(),
        })
    }
}

string_impls_for!(KubeReservedMemoryMinValue, "KubeReservedMemoryMinValue");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_integer_bytes_and_binary_units() {
        for (input, bytes) in [
            ("0", 0),
            ("0Mi", 0),
            ("1", 1),
            ("001", 1),
            ("128Mi", 128 << 20),
            ("200Mi", 200 << 20),
            ("1Ki", 1024),
            ("2Gi", 2 << 30),
            ("9223372036854775807", KubeReservedMemoryMinValue::MAX_BYTES),
            ("9007199254740991Ki", (1_u64 << 63) - 1024),
        ] {
            let value = KubeReservedMemoryMinValue::try_from(input).unwrap();
            assert_eq!(value.as_bytes(), bytes, "{input}");
            assert_eq!(serde_json::to_value(&value).unwrap(), input);
            assert_eq!(
                serde_json::from_value::<KubeReservedMemoryMinValue>(serde_json::json!(input))
                    .unwrap(),
                value
            );
        }
    }

    #[test]
    fn two_maximum_services_have_a_representable_parent() {
        let max = KubeReservedMemoryMinValue::MAX_BYTES;
        assert_eq!(
            KubeReservedMemoryMinValue::try_from(max.to_string().as_str())
                .unwrap()
                .as_bytes()
                .checked_add(max),
            Some(u64::MAX - 1)
        );
        for input in [(max + 1).to_string(), "8589934592Gi".into()] {
            assert!(KubeReservedMemoryMinValue::try_from(input.as_str()).is_err());
            assert!(
                serde_json::from_value::<KubeReservedMemoryMinValue>(serde_json::json!(input))
                    .is_err()
            );
        }
        assert_eq!(
            KubeReservedMemoryMinValue::try_from("8589934591Gi")
                .unwrap()
                .as_bytes(),
            (1_u64 << 63) - (1 << 30)
        );
    }

    #[test]
    fn rejects_unsupported_syntax_and_overflow() {
        for input in [
            "",
            "-1Mi",
            "-0",
            "+1",
            "1..2Mi",
            "1.5Gi",
            "16Ei",
            "1G",
            "1Ti",
            "1ki",
            "1KiB",
            "1e3",
            "1E3",
            ".5",
            "1.",
            "1 Mi",
            " 1",
            "1 ",
            "1\n",
            "１",
            "10%",
            "9223372036854775808",
            "18446744073709551615",
            "18446744073709551616",
            "18014398509481984Ki",
            "17592186044416Mi",
            "17179869184Gi",
        ] {
            assert!(
                KubeReservedMemoryMinValue::try_from(input).is_err(),
                "{input}"
            );
            assert!(
                serde_json::from_value::<KubeReservedMemoryMinValue>(serde_json::json!(input))
                    .is_err(),
                "{input}"
            );
        }
        for value in [
            serde_json::json!(128),
            serde_json::json!(true),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<KubeReservedMemoryMinValue>(value).is_err());
        }
    }
}
