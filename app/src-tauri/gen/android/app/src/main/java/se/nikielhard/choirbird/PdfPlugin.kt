package se.nikielhard.choirbird

// LIB-10 on Android: render a PDF's pages to grey PNG files at about 300 DPI
// with Android's PdfRenderer, for on-device note recognition (homr). Rust
// (`android_pdf.rs`) asks for a PDF path and an output folder and reads the
// PNGs back. Same size rules as the iOS renderer (apple_convert.rs): 300 DPI,
// longest side at most 4200 px, white paper.

import android.app.Activity
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.ColorMatrix
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.graphics.pdf.PdfRenderer
import android.os.ParcelFileDescriptor
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileOutputStream

@InvokeArg
class RenderArgs {
  lateinit var pdf: String
  lateinit var out: String
  var dpi: Double = 300.0
  var maxSide: Int = 4200
}

@TauriPlugin
class PdfPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun render(invoke: Invoke) {
    val a = invoke.parseArgs(RenderArgs::class.java)
    // Off the main thread: a page takes a second or two.
    Thread {
      try {
        val outDir = File(a.out).apply { mkdirs() }
        val files = JSArray()
        ParcelFileDescriptor.open(File(a.pdf), ParcelFileDescriptor.MODE_READ_ONLY).use { fd ->
          PdfRenderer(fd).use { r ->
            for (i in 0 until r.pageCount) {
              r.openPage(i).use { page ->
                // Page size is in points (1/72 in).
                var scale = a.dpi / 72.0
                val longest = maxOf(page.width, page.height) * scale
                if (longest > a.maxSide) scale *= a.maxSide / longest
                val w = maxOf(1, Math.round(page.width * scale).toInt())
                val h = maxOf(1, Math.round(page.height * scale).toInt())
                val bmp = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
                bmp.eraseColor(Color.WHITE)
                page.render(bmp, null, null, PdfRenderer.Page.RENDER_MODE_FOR_PRINT)
                // Grey, like the iOS renderer.
                val grey = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
                val paint = Paint().apply {
                  colorFilter = ColorMatrixColorFilter(ColorMatrix().apply { setSaturation(0f) })
                }
                Canvas(grey).drawBitmap(bmp, 0f, 0f, paint)
                bmp.recycle()
                val f = File(outDir, "page-%03d.png".format(i + 1))
                FileOutputStream(f).use { grey.compress(Bitmap.CompressFormat.PNG, 100, it) }
                grey.recycle()
                files.put(f.absolutePath)
              }
            }
          }
        }
        invoke.resolve(JSObject().put("files", files))
      } catch (e: Exception) {
        invoke.reject(e.message ?: "cannot render the PDF")
      }
    }.start()
  }
}
