# kotlinx.serialization, OkHttp, CameraX and ML Kit ship their own rules. The wire types are
# kept whole so a field the desktop sends is never renamed away.
-keep class com.hyprspace.android.net.** { *; }

# ML Kit finds its parts by constructing their registrars by reflection. R8 can't see that, and
# without these the barcode client comes back broken and the scan screen crashed.
-keep class * implements com.google.firebase.components.ComponentRegistrar { <init>(); }
-keep class com.google.mlkit.**.*Registrar { <init>(); }
-keep class com.google.android.gms.internal.mlkit_vision_barcode.** { *; }
