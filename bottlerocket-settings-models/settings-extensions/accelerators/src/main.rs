use bottlerocket_settings_sdk::{BottlerocketSetting, NullMigratorExtensionBuilder};
use settings_extension_accelerators::AcceleratorsSettingsV1;
use std::process::ExitCode;

fn main() -> ExitCode {
    env_logger::init();

    match NullMigratorExtensionBuilder::with_name("accelerators")
        .with_models(vec![BottlerocketSetting::<AcceleratorsSettingsV1>::model()])
        .build()
    {
        Ok(extension) => extension.run(),
        Err(e) => {
            println!("{e}");
            ExitCode::FAILURE
        }
    }
}
