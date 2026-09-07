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
                let memory_gib = memory.parse::<u32>().ok()?;
                Some(
                    matches!(compute, "1" | "2" | "3" | "4" | "7")
                        && memory_gib > 0
                        && memory_gib.to_string() == memory,
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

/// Model-keyed MIG profiles validated while the settings plugin deserializes API input.
///
/// Runtime API writes load the settings plugin for type deserialization, but do not invoke the
/// settings extension's cross-field validation hook. Keeping the compatibility check in this
/// modeled map prevents an incompatible model/profile pair from being committed.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NvidiaAcceleratorMigProfiles(HashMap<NvidiaGpuModel, NvidiaAcceleratorMigProfile>);

impl<'de> Deserialize<'de> for NvidiaAcceleratorMigProfiles {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let profiles =
            HashMap::<NvidiaGpuModel, NvidiaAcceleratorMigProfile>::deserialize(deserializer)?;
        for (model, profile) in &profiles {
            if !is_approved_profile(model.as_ref(), profile.as_ref()) {
                return Err(serde::de::Error::custom(format!(
                    "NVIDIA MIG profile '{}' is not approved for GPU model '{}'",
                    profile, model
                )));
            }
        }
        Ok(Self(profiles))
    }
}

impl NvidiaAcceleratorMigProfiles {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl IntoIterator for NvidiaAcceleratorMigProfiles {
    type Item = (NvidiaGpuModel, NvidiaAcceleratorMigProfile);
    type IntoIter =
        std::collections::hash_map::IntoIter<NvidiaGpuModel, NvidiaAcceleratorMigProfile>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[model(impl_default = true)]
pub struct NvidiaAcceleratorMigSettings {
    profile: NvidiaAcceleratorMigProfiles,
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
    #[snafu(display(
        "NVIDIA MIG profile '{}' is not approved for GPU model '{}'",
        profile,
        model
    ))]
    UnsupportedMigProfileForModel { model: String, profile: String },
}

fn is_approved_profile(model: &str, profile: &str) -> bool {
    match model {
        "a100.40gb" => matches!(
            profile,
            "1g.5gb" | "1g.10gb" | "2g.10gb" | "3g.20gb" | "7g.40gb"
        ),
        "a100.80gb" | "h100.80gb" => matches!(
            profile,
            "1g.10gb" | "1g.20gb" | "2g.20gb" | "3g.40gb" | "7g.80gb"
        ),
        "h200.141gb" => matches!(
            profile,
            "1g.18gb" | "1g.35gb" | "2g.35gb" | "3g.71gb" | "7g.141gb"
        ),
        "b200.180gb" => matches!(
            profile,
            "1g.23gb" | "1g.45gb" | "2g.45gb" | "3g.90gb" | "7g.180gb"
        ),
        "rtxpro6000.96gb" => matches!(profile, "1g.24gb" | "2g.48gb" | "4g.96gb"),
        "b300.269gb" => matches!(
            profile,
            "1g.34gb" | "1g.67gb" | "2g.67gb" | "3g.135gb" | "4g.135gb" | "7g.269gb"
        ),
        _ => false,
    }
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
                    .clone()
                    .and_then(|mig| mig.profile)
                    .is_some_and(|profile| !profile.is_empty());
                ensure!(has_profile, MissingSharedInferenceMigProfileSnafu);
            }

            if let Some(profiles) = nvidia.mig.and_then(|mig| mig.profile) {
                for (model, profile) in profiles {
                    ensure!(
                        is_approved_profile(model.as_ref(), profile.as_ref()),
                        UnsupportedMigProfileForModelSnafu {
                            model: model.to_string(),
                            profile: profile.to_string(),
                        }
                    );
                }
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
            "1", "7", "0g.10gb", "1g.0", "1g.010gb", "1g.10GB", "1g10gb", "5g.40gb", "6g.40gb", "",
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
    fn deserialization_rejects_profile_incompatible_with_known_model() {
        let json = r#"{"nvidia":{"mode":"dra-shared-inference","mig":{"profile":{"a100.40gb":"1g.18gb"}}}}"#;
        let error = serde_json::from_str::<AcceleratorsSettingsV1>(json).unwrap_err();

        assert!(error
            .to_string()
            .contains("NVIDIA MIG profile '1g.18gb' is not approved for GPU model 'a100.40gb'"));
    }

    #[test]
    fn validation_rejects_profile_incompatible_with_known_model() {
        let settings = AcceleratorsSettingsV1 {
            nvidia: Some(NvidiaAcceleratorSettings {
                mode: Some(NvidiaAcceleratorMode::DraSharedInference),
                mig: Some(NvidiaAcceleratorMigSettings {
                    profile: Some(NvidiaAcceleratorMigProfiles(HashMap::from([(
                        serde_json::from_str(r#""a100.40gb""#).unwrap(),
                        NvidiaAcceleratorMigProfile::try_from("1g.18gb").unwrap(),
                    )]))),
                }),
            }),
        };
        assert_eq!(
            AcceleratorsSettingsV1::validate(settings, None),
            Err(AcceleratorValidationError::UnsupportedMigProfileForModel {
                model: "a100.40gb".to_string(),
                profile: "1g.18gb".to_string(),
            })
        );
    }

    #[test]
    fn rejects_unrecognized_gpu_model_for_accelerator_lifecycle() {
        let json = r#"{"nvidia":{"mode":"dra-shared-inference","mig":{"profile":{"future9000.192gb":"1g.24gb"}}}}"#;
        let error = serde_json::from_str::<AcceleratorsSettingsV1>(json).unwrap_err();

        assert!(error.to_string().contains(
            "NVIDIA MIG profile '1g.24gb' is not approved for GPU model 'future9000.192gb'"
        ));
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
