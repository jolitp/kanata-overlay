// QGuiApplication statics that cxx-qt-lib does not wrap.
#pragma once

#include <QtCore/QString>

QString kog_platform_name();
void kog_set_desktop_file_name(const QString &name);
