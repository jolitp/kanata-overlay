#include "qt_extras.h"

#include <QtGui/QGuiApplication>

QString kog_platform_name()
{
    return QGuiApplication::platformName();
}

void kog_set_desktop_file_name(const QString &name)
{
    QGuiApplication::setDesktopFileName(name);
}
