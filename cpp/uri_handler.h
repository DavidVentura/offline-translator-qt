#pragma once

#include <QtCore/QObject>
#include <QtCore/QStringList>
#include <QtCore/QVariantMap>

using UriCallback = void (*)(void *context, const char *uri);

extern "C" void translator_install_uri_handler(UriCallback callback, void *context);

// lomiri-app-launch hands a URI to an already-running app by walking the
// session bus for connections whose PID matches the app's and calling
// org.freedesktop.Application.Open on an object path derived from $APP_ID.
// Needs Q_OBJECT for the exported slot, so it can't live in a `cpp!` block.
class UriHandler : public QObject {
    Q_OBJECT
    // Without this QtDBus exports the slot under "local.UriHandler" and answers
    // the dispatcher's call with UnknownInterface.
    Q_CLASSINFO("D-Bus Interface", "org.freedesktop.Application")
public:
    UriHandler(UriCallback callback, void *context, QObject *parent);

public slots:
    void Open(const QStringList &uris, const QVariantMap &platformData);

private:
    UriCallback m_callback;
    void *m_context;
};
