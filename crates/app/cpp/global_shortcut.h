// Thin shim over KF6 KGlobalAccel (research R5).
//
// Registers one system-wide shortcut. When it fires, the receiver's
// `activate()` invokable is called; no key events are ever seen here.
// Returns false if the shortcut could not be registered or has no key (e.g.
// the default is already taken by another shortcut).
#pragma once

#include <QtCore/QObject>
#include <QtCore/QString>

bool kog_register_global_shortcut(QObject &receiver, const QString &id,
                                  const QString &text,
                                  const QString &defaultKeys);
