#include "global_shortcut.h"

#include <KGlobalAccel>
#include <algorithm>
#include <QtCore/QMetaObject>
#include <QtGui/QAction>
#include <QtGui/QKeySequence>

bool kog_register_global_shortcut(QObject &receiver, const QString &id,
                                  const QString &text,
                                  const QString &defaultKeys)
{
    auto *action = new QAction(&receiver);
    action->setObjectName(id);
    action->setText(text);
    action->setProperty("componentName", QStringLiteral("kanata-overlay"));
    action->setProperty("componentDisplayName", QStringLiteral("Kanata Overlay"));

    QObject::connect(action, &QAction::triggered, &receiver, [&receiver] {
        QMetaObject::invokeMethod(&receiver, "activate", Qt::QueuedConnection);
    });

    // Sets the default and loads the user's own binding if they changed it.
    if (!KGlobalAccel::setGlobalShortcut(
            action, QKeySequence(defaultKeys, QKeySequence::PortableText))) {
        return false;
    }
    // If the default clashes with another shortcut, KGlobalAccel keeps the
    // action but gives it no key; that is "unavailable" for the user too.
    // shortcut(action) is a local cache; ask the daemon what is really bound.
    // An unbound action comes back as a list holding one empty sequence.
    const auto bound =
        KGlobalAccel::self()->globalShortcut(QStringLiteral("kanata-overlay"), id);
    return std::any_of(bound.cbegin(), bound.cend(),
                       [](const QKeySequence &seq) { return !seq.isEmpty(); });
}
