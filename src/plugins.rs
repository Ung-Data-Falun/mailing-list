use color_eyre::eyre::Result;
use dlopen::wrapper::{Container, WrapperApi};

#[allow(improper_ctypes_definitions)]
#[derive(WrapperApi)]
pub struct PluginApi {
    get_plugin: fn() -> mlpa::Plugin,
}

pub fn get_plugin(plugin: &str) -> Result<(mlpa::Plugin, Container<PluginApi>), dlopen::Error> {
    let plugin_container: Container<PluginApi> = unsafe { Container::load(plugin) }?;
    let plugin = plugin_container.get_plugin();
    Ok((plugin, plugin_container))
}
