// QGuiApplication statics that cxx-qt-lib does not wrap.
#pragma once

#include <QtCore/QString>

#include "rust/cxx.h"

QString kog_platform_name();
void kog_set_desktop_file_name(const QString &name);

// A tray icon (feature 007, research R2): `label` drawn on a rounded square in
// the layer's colour (`r`, `g`, `b`) when connected, grey when not;
// `size`×`size` pixels as straight ARGB32 in network byte order (what
// StatusNotifierItem expects).
rust::Vec<uint8_t> kog_tray_icon(const QString &label, bool connected, int r, int g, int b, int size);
