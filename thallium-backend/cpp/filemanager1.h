#pragma once

#include <QtCore/QStringList>
#include <QtDBus/QDBusAbstractAdaptor>

class BackendBridge;

namespace thallium {

// org.freedesktop.FileManager1, which browsers and other applications call
// for "Show in folder". Each call becomes an externalRequest signal on the
// bridge, carrying one URI and what to do with it.
class FileManager1 : public QDBusAbstractAdaptor
{
	Q_OBJECT
	Q_CLASSINFO("D-Bus Interface", "org.freedesktop.FileManager1")

public:
	explicit FileManager1(QObject* bridge);

public Q_SLOTS:
	void ShowFolders(const QStringList& uris, const QString& startupId);
	void ShowItems(const QStringList& uris, const QString& startupId);
	void ShowItemProperties(const QStringList& uris, const QString& startupId);

private:
	void forward(const QStringList& uris, const QString& mode);
	QObject* m_bridge;
};

// Serve FileManager1 for `bridge`. False when another file manager already
// owns the name, which is not an error: that one answers instead.
bool serveFileManager(BackendBridge& bridge);

}
