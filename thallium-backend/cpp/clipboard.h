#pragma once

#include <QtCore/QString>

namespace thallium {

// Offer files on the system clipboard the way GTK, KDE and Qt file managers
// read them. `uris` is newline separated.
void clipboardOfferFiles(const QString& uris, bool cut);

// The files on the clipboard as "copy" or "cut" on the first line and one URI
// per line after it, or an empty string when the clipboard holds no files.
QString clipboardFiles();

// Drop what was cut once it has been moved, so a second paste cannot fail.
void clipboardClear();

}
