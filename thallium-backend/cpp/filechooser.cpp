#include "thallium-backend/filechooser.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QProcess>
#include <QtDBus/QDBusConnection>

namespace thallium {

namespace {

// Paths arrive as NUL-terminated byte arrays.
QString bytePath(const QVariant& value)
{
	QByteArray bytes = value.toByteArray();
	while (bytes.endsWith('\0'))
		bytes.chop(1);
	return QString::fromLocal8Bit(bytes);
}

}

FileChooser::FileChooser(QObject* parent)
	: QDBusAbstractAdaptor(parent)
{
}

uint FileChooser::OpenFile(const QDBusObjectPath&, const QString&, const QString&,
						   const QString& title, const QVariantMap& options,
						   const QDBusMessage& message, QVariantMap&)
{
	const QString mode = options.value(QStringLiteral("directory")).toBool() ? QStringLiteral("folder")
		: options.value(QStringLiteral("multiple")).toBool()                 ? QStringLiteral("open-multiple")
																			 : QStringLiteral("open");
	pick(mode, title, options, message);
	return 0;
}

uint FileChooser::SaveFile(const QDBusObjectPath&, const QString&, const QString&,
						   const QString& title, const QVariantMap& options,
						   const QDBusMessage& message, QVariantMap&)
{
	pick(QStringLiteral("save"), title, options, message);
	return 0;
}

void FileChooser::pick(const QString& mode, const QString& title, const QVariantMap& options,
					   const QDBusMessage& message)
{
	message.setDelayedReply(true);

	QStringList args { QStringLiteral("--pick"), mode, QStringLiteral("--pick-title"), title };
	QString folder = bytePath(options.value(QStringLiteral("current_folder")));
	const QString file = bytePath(options.value(QStringLiteral("current_file")));
	QString name = options.value(QStringLiteral("current_name")).toString();
	if (!file.isEmpty()) {
		folder = file.section(QLatin1Char('/'), 0, -2);
		name = file.section(QLatin1Char('/'), -1);
	}
	if (!folder.isEmpty())
		args << QStringLiteral("--pick-folder") << folder;
	if (!name.isEmpty())
		args << QStringLiteral("--pick-name") << name;
	const QString accept = options.value(QStringLiteral("accept_label")).toString();
	if (!accept.isEmpty())
		args << QStringLiteral("--pick-accept") << accept;

	auto* process = new QProcess(this);
	process->setProcessChannelMode(QProcess::ForwardedErrorChannel);
	connect(process, &QProcess::finished, this, [process, message](int, QProcess::ExitStatus) {
		QStringList uris;
		for (const QByteArray& line : process->readAllStandardOutput().split('\n'))
			if (!line.trimmed().isEmpty())
				uris << QString::fromUtf8(line.trimmed());
		process->deleteLater();
		// 0 is success, 1 is cancelled by the user.
		QVariantMap results;
		if (!uris.isEmpty())
			results.insert(QStringLiteral("uris"), uris);
		QDBusConnection::sessionBus().send(
			message.createReply({ uint(uris.isEmpty() ? 1 : 0), results }));
	});
	process->start(QCoreApplication::applicationFilePath(), args);
}

bool servePortal()
{
	QDBusConnection bus = QDBusConnection::sessionBus();
	if (!bus.isConnected())
		return false;
	auto* root = new QObject(QCoreApplication::instance());
	new FileChooser(root);
	if (!bus.registerObject(QStringLiteral("/org/freedesktop/portal/desktop"), root))
		return false;
	return bus.registerService(QStringLiteral("org.freedesktop.impl.portal.desktop.thallium"));
}

}
