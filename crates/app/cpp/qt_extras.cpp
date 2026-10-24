#include "qt_extras.h"

#include <QtGui/QFont>
#include <QtGui/QFontMetrics>
#include <QtGui/QGuiApplication>
#include <QtGui/QImage>
#include <QtGui/QPainter>

QString kog_platform_name()
{
    return QGuiApplication::platformName();
}

void kog_set_desktop_file_name(const QString &name)
{
    QGuiApplication::setDesktopFileName(name);
}

rust::Vec<uint8_t> kog_tray_icon(const QString &label, bool connected, int r, int g, int b, int size)
{
    QImage image(size, size, QImage::Format_ARGB32);
    image.fill(Qt::transparent);
    {
        QPainter p(&image);
        p.setRenderHint(QPainter::Antialiasing);
        p.setPen(Qt::NoPen);
        p.setBrush(connected ? QColor(r, g, b) : QColor(0x8a, 0x8a, 0x8a));
        p.drawRoundedRect(QRectF(0.5, 0.5, size - 1, size - 1), size * 0.2, size * 0.2);

        // The largest bold font in which the label fits, with a small margin.
        QFont font;
        font.setBold(true);
        int px = int(size * 0.62);
        for (; px > 6; --px) {
            font.setPixelSize(px);
            if (QFontMetrics(font).horizontalAdvance(label) <= size - 4)
                break;
        }
        font.setPixelSize(px);
        p.setFont(font);
        p.setPen(Qt::white);
        p.drawText(QRect(0, 0, size, size), Qt::AlignCenter, label);
    }
    rust::Vec<uint8_t> out;
    out.reserve(size_t(size) * size * 4);
    for (int y = 0; y < size; ++y) {
        const QRgb *line = reinterpret_cast<const QRgb *>(image.constScanLine(y));
        for (int x = 0; x < size; ++x) {
            const QRgb c = line[x];
            out.push_back(uint8_t(qAlpha(c)));
            out.push_back(uint8_t(qRed(c)));
            out.push_back(uint8_t(qGreen(c)));
            out.push_back(uint8_t(qBlue(c)));
        }
    }
    return out;
}
