use color_eyre::eyre::Result;
use dlopen::wrapper::{Container, WrapperApi};

#[allow(improper_ctypes_definitions)]
#[derive(WrapperApi)]
pub struct PluginApi {
    get_plugin: fn() -> mlpa::Plugin,
}

#[derive(Debug, Clone, Copy)]
pub struct UnwrappedPlugin {
    pub message_handler: Option<fn(String)>,
}

pub fn get_plugin(plugin: &str) -> Result<(UnwrappedPlugin, Container<PluginApi>), dlopen::Error> {
    let plugin_container: Container<PluginApi> = unsafe { Container::load(plugin) }?;
    let plugin = plugin_container.get_plugin();
    let plugin = UnwrappedPlugin {
        message_handler: match plugin.message_handler {
            Some(v) => Some(unsafe { *Box::from_raw(v) }),
            None => None,
        },
    };
    Ok((plugin, plugin_container))
}
