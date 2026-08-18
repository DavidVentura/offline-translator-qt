#include "uri_handler.h"

#include <QtCore/QByteArray>
#include <QtCore/QCoreApplication>
#include <QtCore/QDebug>
#include <QtCore/QString>
#include <QtDBus/QDBusConnection>

UriHandler::UriHandler(UriCallback callback, void *context, QObject *parent)
    : QObject(parent), m_callback(callback), m_context(context) {}

void UriHandler::Open(const QStringList &uris, const QVariantMap &platformData) {
    Q_UNUSED(platformData);
    for (const QString &uri : uris) {
        const QByteArray utf8 = uri.toUtf8();
        m_callback(m_context, utf8.constData());
    }
}

// The escaping lomiri-app-launch applies to $APP_ID to pick the object path it
// sends Open to: alphanumerics survive, everything else -- including a leading
// digit -- becomes _<hex>.
static QString objectPathForAppId(const QByteArray &appId) {
    QString path = QStringLiteral("/");
    for (int i = 0; i < appId.size(); ++i) {
        const char c = appId.at(i);
        const bool alnum = (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                           (c >= '0' && c <= '9' && i != 0);
        if (alnum) {
            path += QLatin1Char(c);
        } else {
            path += QString::asprintf("_%02x", c);
        }
    }
    return path;
}

extern "C" void translator_install_uri_handler(UriCallback callback, void *context) {
    const QByteArray appId = qgetenv("APP_ID");
    if (appId.isEmpty()) {
        qWarning() << "uri handler: no APP_ID, not listening for dispatched URIs";
        return;
    }

    QDBusConnection session = QDBusConnection::sessionBus();
    if (!session.isConnected()) {
        qWarning() << "uri handler: no session bus, not listening for dispatched URIs";
        return;
    }

    const QString path = objectPathForAppId(appId);
    UriHandler *handler = new UriHandler(callback, context, QCoreApplication::instance());
    if (!session.registerObject(path, handler, QDBusConnection::ExportAllSlots)) {
        qWarning() << "uri handler: could not register" << path;
        return;
    }
    qDebug() << "uri handler: listening on" << path;
}
