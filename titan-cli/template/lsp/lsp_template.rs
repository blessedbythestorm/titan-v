use ad_astra::{
    export,
    runtime::{
        ops::{DynamicArgument, DynamicReturn, DynamicType},
        ScriptPackage,
    },
    server::{
        inlay_hint, LspLoggerConfig, LspLoggerServerConfig, LspServer, LspServerConfig,
        LspTransportConfig,
    },
};

#[export(package)]
#[derive(Default)]
pub struct LspPackage;

#[export]
pub fn dbg(x: DynamicArgument<DynamicType>) -> DynamicReturn<DynamicType> {
    let message = x.data.stringify(false);
    let tooltip = x.data.stringify(true);

    // debug!("{}", tooltip);

    let tooltip = match message == tooltip {
        true => String::new(),
        false => format!("```\n{tooltip}\n```"),
    };

    inlay_hint(x.origin, message, tooltip);

    DynamicReturn::new(x.data)
}

fn main() {
    let server_config = LspServerConfig::new();

    let mut logger_config = LspLoggerConfig::new();

    let transport_config = LspTransportConfig::Stdio;

    logger_config.server = LspLoggerServerConfig::Off;

    LspServer::startup(
        server_config,
        logger_config,
        transport_config,
        LspPackage::meta(),
    );
}
