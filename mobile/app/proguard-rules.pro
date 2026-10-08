# kotlinx.serialization, OkHttp, CameraX and ML Kit ship their own rules. The wire types are
# kept whole so a field the desktop sends is never renamed away.
-keep class com.hyprspace.android.net.** { *; }
