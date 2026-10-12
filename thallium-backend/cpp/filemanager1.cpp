#include "thallium-backend/filemanager1.h"

#include <QtDBus/QDBusConnection>

#include "thallium-backend/src/qt_bridge.cxxqt.h"

namespace thallium {

FileManager1::FileManager1(QObject* bridge)
	: QDBusAbstractAdaptor(bridge)
	, m_bridge(bridge)
{
}

void FileManager1::ShowFolders(const QStringList& uris, const QString&)
{
	forward(uris, QStringLiteral("folder"));
}

void FileManager1::ShowItems(const QStringList& uris, const QString&)
{
	forward(uris, QStringLiteral("item"));
}

void FileManager1::ShowItemProperties(const QStringList& uris, const QString&)
{
	forward(uris, QStringLiteral("properties"));
}

void FileManager1::forward(const QStringList& uris, const QString& mode)
{
	for (const QString& uri : uris)
		QMetaObject::invokeMethod(m_bridge, "externalRequest", Qt::QueuedConnection,
								  Q_ARG(QString, uri), Q_ARG(QString, mode));
}

bool serveFileManager(BackendBridge& bridge)
{
	QDBusConnection bus = QDBusConnection::sessionBus();
	if (!bus.isConnected())
		return false;
	new FileManager1(&bridge);
	if (!bus.registerObject(QStringLiteral("/org/freedesktop/FileManager1"), &bridge))
		return false;
	return bus.registerService(QStringLiteral("org.freedesktop.FileManager1"));
}

}
