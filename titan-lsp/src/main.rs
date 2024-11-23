use ad_astra::{
    runtime::{
        //     ops::{DynamicArgument, DynamicReturn, DynamicType},
        ScriptPackage,
    },
    server::{
        LspLoggerConfig, LspLoggerServerConfig, LspServer, LspServerConfig, LspTransportConfig,
    },
};

use titan_viewer::ViewerPackage;

// #[export(package)]
// #[derive(Default)]
// pub struct TitanLSP;

// #[export]
// pub fn dbg(x: DynamicArgument<DynamicType>) -> DynamicReturn<DynamicType> {
//     let message = x.data.stringify(false);
//     let tooltip = x.data.stringify(true);

//     // debug!("{}", tooltip);

//     let tooltip = match message == tooltip {
//         true => String::new(),
//         false => format!("```\n{tooltip}\n```"),
//     };

//     inlay_hint(x.origin, message, tooltip);

//     DynamicReturn::new(x.data)
// }

fn main() {
    let server_config = LspServerConfig::new();

    let mut logger_config = LspLoggerConfig::new();

    let transport_config = LspTransportConfig::Stdio;

    logger_config.server = LspLoggerServerConfig::Off;

    LspServer::startup(
        server_config,
        logger_config,
        transport_config,
        ViewerPackage::meta(),
    );
}
