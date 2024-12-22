use ad_astra::{
    runtime::{
        ScriptPackage,
    },
    server::{
        LspLoggerConfig, LspLoggerServerConfig, LspServer, LspServerConfig, LspTransportConfig,
    },
};

use titan_viewer::ViewerPackage;

fn main() {
    let server_config = LspServerConfig::new();

    let mut logger_config = LspLoggerConfig::new();

    logger_config.server = LspLoggerServerConfig::Off;

    LspServer::startup(
        server_config,
        logger_config,
        LspTransportConfig::Stdio,
        ViewerPackage::meta(),
    );
}
