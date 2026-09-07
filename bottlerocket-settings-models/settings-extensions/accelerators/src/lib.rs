//! Settings for selecting the node-local accelerator resource provider and profile.

use bottlerocket_model_derive::model;
use bottlerocket_modeled_types::NvidiaGpuModel;
use bottlerocket_settings_sdk::{GenerateResult, SettingsModel};
use bottlerocket_string_impls_for::string_impls_for;
use serde::{Deserialize, Serialize};
use snafu::{ensure, Snafu};
use std::collections::HashMap;

/// One complete NVIDIA accelerator mode.
///
/// Provider ownership and DRA profile selection intentionally share one enum so conflicting
/// providers and incomplete DRA configurations cannot be represented.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NvidiaAcceleratorMode {
    Disabled,
    DevicePlugin,
    DraSharedInference,
    DraDistributedTraining,
}

/// An explicit NVIDIA MIG profile used by the accelerator lifecycle.
///
/// Numeric aliases accepted by the legacy device-plugin settings are intentionally rejected:
/// lifecycle validation must be able to compare the requested geometry with the actual devices.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NvidiaAcceleratorMigProfile {
    inner: String,
}

#[derive(Debug, Snafu, PartialEq)]
pub enum NvidiaAcceleratorMigProfileError {
    #[snafu(display(
        "invalid NVIDIA accelerator MIG profile '{}'; expected an explicit profile like 1g.10gb",
        value
    ))]
    InvalidProfile { value: String },
}

impl TryFrom<&str> for NvidiaAcceleratorMigProfile {
    type Error = NvidiaAcceleratorMigProfileError;

    fn try_from(input: &str) -> std::result::Result<Self, Self::Error> {
        let valid = input
            .split_once("g.")
            .and_then(|(compute, memory)| {
                let memory = memory.strip_suffix("gb")?;
                Some(
                    matches!(compute, "1" | "2" | "3" | "4" | "7")
                        && memory
                            .parse::<u32>()
                            .ok()
                            .is_some_and(|memory_gib| memory_gib > 0),
                )
            })
            .unwrap_or(false);
        ensure!(
            valid,
            InvalidProfileSnafu {
                value: input.to_string()
            }
        );
        Ok(Self {
            inner: input.to_string(),
        })
    }
}

string_impls_for!(NvidiaAcceleratorMigProfile, "NvidiaAcceleratorMigProfile");

#[model(impl_default = true)]
pub struct NvidiaAcceleratorMigSettings {
    profile: HashMap<NvidiaGpuModel, NvidiaAcceleratorMigProfile>,
}

#[model(impl_default = true)]
pub struct NvidiaAcceleratorSettings {
    mode: NvidiaAcceleratorMode,
    mig: NvidiaAcceleratorMigSettings,
}

#[model(impl_default = true)]
pub struct AcceleratorsSettingsV1 {
    nvidia: NvidiaAcceleratorSettings,
}

#[derive(Debug, Snafu, PartialEq)]
pub enum AcceleratorValidationError {
    #[snafu(display("dra-shared-inference requires at least one approved NVIDIA MIG profile"))]
    MissingSharedInferenceMigProfile,
}

type Result<T> = std::result::Result<T, AcceleratorValidationError>;

impl SettingsModel for AcceleratorsSettingsV1 {
    type PartialKind = Self;
    type ErrorKind = AcceleratorValidationError;

    fn get_version() -> &'static str {
        "v1"
    }

    fn set(_current_value: Option<Self>, _target: Self) -> Result<()> {
        Ok(())
    }

    fn generate(
        existing_partial: Option<Self::PartialKind>,
        _dependent_settings: Option<serde_json::Value>,
    ) -> Result<GenerateResult<Self::PartialKind, Self>> {
        Ok(GenerateResult::Complete(
            existing_partial.unwrap_or_default(),
        ))
    }

    fn validate(value: Self, _validated_settings: Option<serde_json::Value>) -> Result<()> {
        if let Some(nvidia) = value.nvidia {
            if nvidia.mode == Some(NvidiaAcceleratorMode::DraSharedInference) {
                let has_profile = nvidia
                    .mig
                    .and_then(|mig| mig.profile)
                    .is_some_and(|profile| !profile.is_empty());
                ensure!(has_profile, MissingSharedInferenceMigProfileSnafu);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_empty_settings() {
        assert_eq!(
            AcceleratorsSettingsV1::generate(None, None).unwrap(),
            GenerateResult::Complete(AcceleratorsSettingsV1 { nvidia: None })
        );
    }

    #[test]
    fn accepts_each_nvidia_mode() {
        for (serialized, mode) in [
            ("disabled", NvidiaAcceleratorMode::Disabled),
            ("device-plugin", NvidiaAcceleratorMode::DevicePlugin),
            (
                "dra-shared-inference",
                NvidiaAcceleratorMode::DraSharedInference,
            ),
            (
                "dra-distributed-training",
                NvidiaAcceleratorMode::DraDistributedTraining,
            ),
        ] {
            let json = format!(r#"{{"nvidia":{{"mode":"{serialized}"}}}}"#);
            let settings: AcceleratorsSettingsV1 = serde_json::from_str(&json).unwrap();

            assert_eq!(
                settings,
                AcceleratorsSettingsV1 {
                    nvidia: Some(NvidiaAcceleratorSettings {
                        mode: Some(mode),
                        mig: None,
                    }),
                }
            );
            assert_eq!(serde_json::to_string(&settings).unwrap(), json);
        }
    }

    #[test]
    fn rejects_unknown_nvidia_mode() {
        let result = serde_json::from_str::<AcceleratorsSettingsV1>(r#"{"nvidia":{"mode":"dra"}}"#);

        assert!(result.is_err());
    }

    #[test]
    fn accepts_explicit_mig_profiles() {
        for profile in [
            "1g.5gb", "1g.10gb", "2g.20gb", "3g.40gb", "4g.96gb", "7g.141gb",
        ] {
            assert!(NvidiaAcceleratorMigProfile::try_from(profile).is_ok());
        }
    }

    #[test]
    fn rejects_ambiguous_or_malformed_mig_profiles() {
        for profile in [
            "1", "7", "0g.10gb", "1g.0", "1g.10GB", "1g10gb", "5g.40gb", "6g.40gb", "",
        ] {
            assert!(NvidiaAcceleratorMigProfile::try_from(profile).is_err());
        }
    }

    #[test]
    fn shared_inference_requires_an_approved_geometry() {
        let settings: AcceleratorsSettingsV1 =
            serde_json::from_str(r#"{"nvidia":{"mode":"dra-shared-inference"}}"#).unwrap();

        assert_eq!(
            AcceleratorsSettingsV1::validate(settings, None),
            Err(AcceleratorValidationError::MissingSharedInferenceMigProfile)
        );
    }

    #[test]
    fn shared_inference_accepts_a_model_keyed_geometry() {
        let json = r#"{"nvidia":{"mode":"dra-shared-inference","mig":{"profile":{"h100.80gb":"1g.10gb"}}}}"#;
        let settings: AcceleratorsSettingsV1 = serde_json::from_str(json).unwrap();

        assert!(AcceleratorsSettingsV1::validate(settings.clone(), None).is_ok());
        assert_eq!(serde_json::to_string(&settings).unwrap(), json);
    }

    #[test]
    fn other_modes_may_keep_geometry_for_a_future_transition() {
        for mode in ["disabled", "device-plugin", "dra-distributed-training"] {
            let json = format!(
                r#"{{"nvidia":{{"mode":"{mode}","mig":{{"profile":{{"h100.80gb":"1g.10gb"}}}}}}}}"#
            );
            let settings: AcceleratorsSettingsV1 = serde_json::from_str(&json).unwrap();
            assert!(AcceleratorsSettingsV1::validate(settings, None).is_ok());
        }
    }

    #[test]
    fn rejects_independent_provider_flags() {
        let result = serde_json::from_str::<AcceleratorsSettingsV1>(
            r#"{"nvidia":{"device-plugin-enabled":true,"dra-enabled":true}}"#,
        );

        assert!(result.is_err());
    }
}
