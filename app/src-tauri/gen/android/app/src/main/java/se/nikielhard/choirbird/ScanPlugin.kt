package se.nikielhard.choirbird

// LIB-12: scan paper sheet music with Google's ML Kit document scanner (edge
// detection, crop, perspective fix; runs on the device through Google Play
// services). Returns the pages as base64 JPEG to Rust (`scan.rs`), which
// writes the PDF.

import android.app.Activity
import android.util.Base64
import androidx.activity.result.ActivityResult
import androidx.activity.result.IntentSenderRequest
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.mlkit.vision.documentscanner.GmsDocumentScannerOptions
import com.google.mlkit.vision.documentscanner.GmsDocumentScanning
import com.google.mlkit.vision.documentscanner.GmsDocumentScanningResult

@TauriPlugin
class ScanPlugin(private val activity: Activity) : Plugin(activity) {
  /** ML Kit's scanner needs Google Play services and a camera. */
  @Command
  fun available(invoke: Invoke) {
    val pm = activity.packageManager
    val camera = pm.hasSystemFeature(android.content.pm.PackageManager.FEATURE_CAMERA_ANY)
    val gms = GoogleApiAvailability.getInstance()
      .isGooglePlayServicesAvailable(activity) == ConnectionResult.SUCCESS
    invoke.resolve(JSObject().put("available", camera && gms))
  }

  @Command
  fun scan(invoke: Invoke) {
    val options = GmsDocumentScannerOptions.Builder()
      .setGalleryImportAllowed(true)
      .setResultFormats(GmsDocumentScannerOptions.RESULT_FORMAT_JPEG)
      .setScannerMode(GmsDocumentScannerOptions.SCANNER_MODE_FULL)
      .build()
    GmsDocumentScanning.getClient(options)
      .getStartScanIntent(activity)
      .addOnSuccessListener { sender ->
        startIntentSenderForResult(invoke, IntentSenderRequest.Builder(sender).build(), "scanned")
      }
      .addOnFailureListener { e -> invoke.reject(e.message ?: "scanner unavailable") }
  }

  @ActivityCallback
  private fun scanned(invoke: Invoke, result: ActivityResult) {
    val res = GmsDocumentScanningResult.fromActivityResultIntent(result.data)
    if (result.resultCode != Activity.RESULT_OK || res == null) {
      invoke.resolve(JSObject().put("pages", JSArray()))
      return
    }
    try {
      val pages = JSArray()
      for (p in res.pages ?: emptyList()) {
        val bytes = activity.contentResolver.openInputStream(p.imageUri)?.use { it.readBytes() }
          ?: continue
        pages.put(Base64.encodeToString(bytes, Base64.NO_WRAP))
      }
      invoke.resolve(JSObject().put("pages", pages))
    } catch (e: Exception) {
      invoke.reject(e.message ?: "could not read the scanned pages")
    }
  }
}
