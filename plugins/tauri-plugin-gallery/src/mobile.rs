use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::{SaveRequest, SaveResponse};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "aiimage.gallery";

/// 移动端实现：持有插件句柄，经 run_mobile_plugin 调用 Kotlin 侧
pub struct Gallery<R: Runtime>(PluginHandle<R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Gallery<R>> {
    #[cfg(target_os = "android")]
    let handle = api
        .register_android_plugin(PLUGIN_IDENTIFIER, "GalleryPlugin")
        .map_err(|e| crate::Error::Invoke(e.to_string()))?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_gallery)?;
    Ok(Gallery(handle))
}

impl<R: Runtime> Gallery<R> {
    pub fn save_to_gallery(&self, payload: SaveRequest) -> crate::Result<SaveResponse> {
        self.0
            .run_mobile_plugin("saveToGallery", payload)
            .map_err(|e| crate::Error::Invoke(e.to_string()))
    }
}
