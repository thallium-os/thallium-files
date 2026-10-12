#include "clipboard.h"

#include <QtCore/QMimeData>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>

namespace thallium {

namespace {
constexpr auto gnomeFiles = "x-special/gnome-copied-files";
constexpr auto kdeCut = "application/x-kde-cutselection";
}

void clipboardOfferFiles(const QString& uris, bool cut)
{
	const QStringList lines = uris.split(u'\n', Qt::SkipEmptyParts);
	QList<QUrl> urls;
	QStringList paths;
	for (const QString& line : lines) {
		const QUrl url(line, QUrl::StrictMode);
		urls.append(url);
		paths.append(url.toLocalFile());
	}
	auto* data = new QMimeData;
	data->setUrls(urls);
	data->setText(paths.join(u'\n'));
	const QString gnome = QStringLiteral("%1\n%2").arg(cut ? u"cut" : u"copy", lines.join(u'\n'));
	data->setData(gnomeFiles, gnome.toUtf8());
	data->setData(kdeCut, cut ? "1" : "0");
	QGuiApplication::clipboard()->setMimeData(data);
}

QString clipboardFiles()
{
	const QMimeData* data = QGuiApplication::clipboard()->mimeData();
	if (data == nullptr)
		return {};
	if (data->hasFormat(gnomeFiles)) {
		const QString text = QString::fromUtf8(data->data(gnomeFiles));
		if (text.startsWith(u"copy\n") || text.startsWith(u"cut\n"))
			return text;
	}
	if (!data->hasUrls())
		return {};
	const bool cut = data->data(kdeCut) == "1";
	QStringList lines { cut ? QStringLiteral("cut") : QStringLiteral("copy") };
	for (const QUrl& url : data->urls())
		lines.append(QString::fromUtf8(url.toEncoded()));
	return lines.join(u'\n');
}

void clipboardClear()
{
	QGuiApplication::clipboard()->clear();
}

}
