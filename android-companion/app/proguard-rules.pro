# SPS Companion ProGuard rules

# Keep Kotlin metadata (reflection used by serialization).
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt

# Keep all @Serializable classes.
-keep,includedescriptorclasses class com.sps.companion.**$$serializer { *; }
-keepclassmembers class com.sps.companion.** {
    *** Companion;
}
-keepclasseswithmembers class com.sps.companion.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# TFLite — keep native method signatures.
-keep class org.tensorflow.lite.** { *; }
-keep class org.tensorflow.lite.support.** { *; }
-keep class org.tensorflow.lite.task.** { *; }

# OkHttp — keep internal API.
-dontwarn okhttp3.internal.**
-dontwarn okio.**

# Compose — keep runtime.
-keep class androidx.compose.runtime.** { *; }
