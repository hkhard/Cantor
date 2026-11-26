// LIB-10: homr's note-recognition models (123 MB, AGPL-3.0) as a Play
// install-time asset pack, so the base module stays small. Install-time packs
// are read through the ordinary AssetManager under the same paths
// (resources/homr/...), so android_assets.rs copies them to app data as
// before. The models are not in git: `tools/bundle_homr_models.sh --android`
// copies them into src/main/assets/resources/homr (git-ignored).
plugins {
    id("com.android.asset-pack")
}

assetPack {
    packName.set("homr_models")
    dynamicDelivery {
        deliveryType.set("install-time")
    }
}
