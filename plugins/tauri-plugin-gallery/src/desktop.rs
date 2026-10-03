use serde::{de::DeserializeOwned, Serialize};
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::{Error, SaveRequest, SaveResponse};

/// 桌面端占位实现：相册写入仅存在于移动端
/// （fn() -> R 函数指针恒为 Send + Sync，满足 app.manage 的约束）
pub struct Gallery<R: Runtime>(std::marker::PhantomData<fn() -> R>);

pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Gallery<R>> {
    Ok(Gallery(std::marker::PhantomData))
}

impl<R: Runtime> Gallery<R> {
    pub fn save_to_gallery(&self, _payload: SaveRequest) -> crate::Result<SaveResponse> {
        Err(Error::DesktopNotSupported)
    }
}
