use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{
    plugin::{Builder, PluginApi, PluginHandle},
    AppHandle, Manager, Runtime,
};

mod error;
pub use error::{Error, Result};

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop::Gallery;
#[cfg(mobile)]
use mobile::Gallery;

/// 保存请求（serde 字段转 camelCase 对齐 Kotlin 侧 @InvokeArg）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    /// 图片 base64（不带 data: 前缀）
    pub data_b64: String,
    /// 文件名（含扩展名，决定 MIME 与格式）
    pub filename: String,
}

/// 保存结果
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResponse {
    /// 写入成功后的内容 URI（相册条目）
    pub uri: String,
}

pub trait GalleryExt<R: Runtime> {
    fn gallery(&self) -> &Gallery<R>;
}

impl<R: Runtime, T: Manager<R>> GalleryExt<R> for T {
    fn gallery(&self) -> &Gallery<R> {
        self.state::<Gallery<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    Builder::new("gallery")
        .setup(|app, api| {
            #[cfg(mobile)]
            let gallery = mobile::init(app, api)?;
            #[cfg(desktop)]
            let gallery = desktop::init(app, api)?;
            app.manage(gallery);
            Ok(())
        })
        .build()
}
