use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// 桌面平台不支持相册写入（桌面端直接落盘，不走本插件）
    DesktopNotSupported,
    /// 移动端插件调用失败
    #[cfg(mobile)]
    Invoke(tauri::plugin::PluginInvokeError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::DesktopNotSupported => {
                write!(f, "相册保存仅支持移动端，桌面平台请使用文件保存")
            }
            #[cfg(mobile)]
            Error::Invoke(e) => write!(f, "相册插件调用失败：{e}"),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(mobile)]
impl From<tauri::plugin::PluginInvokeError> for Error {
    fn from(e: tauri::plugin::PluginInvokeError) -> Self {
        Error::Invoke(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
