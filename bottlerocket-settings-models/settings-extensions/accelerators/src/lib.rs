//! Settings for selecting the node-local accelerator resource provider and profile.

use bottlerocket_model_derive::model;
use bottlerocket_settings_sdk::{GenerateResult, SettingsModel};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

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

#[model(impl_default = true)]
pub struct NvidiaAcceleratorSettings {
    mode: NvidiaAcceleratorMode,
}

#[model(impl_default = true)]
pub struct AcceleratorsSettingsV1 {
    nvidia: NvidiaAcceleratorSettings,
}

type Result<T> = std::result::Result<T, Infallible>;

impl SettingsModel for AcceleratorsSettingsV1 {
    type PartialKind = Self;
    type ErrorKind = Infallible;

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

    fn validate(_value: Self, _validated_settings: Option<serde_json::Value>) -> Result<()> {
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
                    nvidia: Some(NvidiaAcceleratorSettings { mode: Some(mode) }),
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
    fn rejects_independent_provider_flags() {
        let result = serde_json::from_str::<AcceleratorsSettingsV1>(
            r#"{"nvidia":{"device-plugin-enabled":true,"dra-enabled":true}}"#,
        );

        assert!(result.is_err());
    }
}
