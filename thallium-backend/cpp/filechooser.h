#pragma once

#include <QtCore/QVariantMap>
#include <QtDBus/QDBusAbstractAdaptor>
#include <QtDBus/QDBusMessage>
#include <QtDBus/QDBusObjectPath>

namespace thallium {

// org.freedesktop.impl.portal.FileChooser, which xdg-desktop-portal calls
// when an application asks for a file dialog. Each request runs
// `thallium-file-manager --pick` and answers with what it printed.
class FileChooser : public QDBusAbstractAdaptor
{
	Q_OBJECT
	Q_CLASSINFO("D-Bus Interface", "org.freedesktop.impl.portal.FileChooser")

public:
	explicit FileChooser(QObject* parent);

public Q_SLOTS:
	uint OpenFile(const QDBusObjectPath& handle, const QString& appId, const QString& parentWindow,
				  const QString& title, const QVariantMap& options, const QDBusMessage& message,
				  QVariantMap& results);
	uint SaveFile(const QDBusObjectPath& handle, const QString& appId, const QString& parentWindow,
				  const QString& title, const QVariantMap& options, const QDBusMessage& message,
				  QVariantMap& results);

private:
	void pick(const QString& mode, const QString& title, const QVariantMap& options,
			  const QDBusMessage& message);
};

// Serve the portal backend until the process is killed. False when the bus
// is missing or another instance already owns the name.
bool servePortal();

}
