package aiimage.gallery

import android.app.Activity
import android.content.ContentValues
import android.os.Build
import android.provider.MediaStore
import android.util.Base64
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class SaveArgs {
    lateinit var dataB64: String
    lateinit var filename: String
}

@TauriPlugin
class GalleryPlugin(private val activity: Activity) : Plugin(activity) {

    @Command
    fun saveToGallery(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(SaveArgs::class.java)
            val bytes = Base64.decode(args.dataB64, Base64.DEFAULT)
            if (bytes.isEmpty()) {
                throw IllegalArgumentException("图片数据为空")
            }

            val lower = args.filename.lowercase()
            val mime =
                if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) "image/jpeg" else "image/png"

            val resolver = activity.contentResolver
            val values = ContentValues().apply {
                put(MediaStore.Images.Media.DISPLAY_NAME, args.filename)
                put(MediaStore.Images.Media.MIME_TYPE, mime)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    // Android 10+：写入公共相册 Pictures/ai-image，无需存储权限
                    put(MediaStore.Images.Media.RELATIVE_PATH, "Pictures/ai-image")
                    put(MediaStore.Images.Media.IS_PENDING, 1)
                }
            }

            val uri = resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)
                ?: throw IllegalStateException("无法在系统相册创建条目")

            resolver.openOutputStream(uri)?.use { it.write(bytes) }
                ?: throw IllegalStateException("无法写入图片数据")

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val done = ContentValues().apply {
                    put(MediaStore.Images.Media.IS_PENDING, 0)
                }
                resolver.update(uri, done, null, null)
            }

            val ret = JSObject()
            ret.put("uri", uri.toString())
            invoke.resolve(ret)
        } catch (e: Exception) {
            invoke.reject(e.message ?: "保存到相册失败")
        }
    }
}
