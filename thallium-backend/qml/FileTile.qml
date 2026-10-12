import QtQuick

// The reference's file mark: a grey tile with a folded corner and the kind of
// file drawn dark on it. A folder is a solid folder shape; a picture or video
// with a thumbnail shows the thumbnail instead.
Item {
    id: tile
    property string name: ""
    property bool folder: false
    property string thumbnail: ""
    property bool lifted: false

    readonly property string extension: {
        const dot = tile.name.lastIndexOf(".")
        return dot > 0 ? tile.name.slice(dot + 1).toLowerCase() : ""
    }
    // Office documents and PDFs carry a letter, as in the reference.
    readonly property string letter: {
        if (["doc", "docx", "odt", "rtf"].indexOf(tile.extension) >= 0) return "W"
        if (["xls", "xlsx", "ods", "csv"].indexOf(tile.extension) >= 0) return "X"
        if (["ppt", "pptx", "odp"].indexOf(tile.extension) >= 0) return "P"
        if (tile.extension === "pdf") return "PDF"
        return ""
    }
    readonly property string glyph: {
        const e = tile.extension
        if (["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "tiff", "heic"].indexOf(e) >= 0) return "file-image"
        if (["mp4", "mkv", "webm", "mov", "avi", "m4v"].indexOf(e) >= 0) return "file-video"
        if (["mp3", "flac", "wav", "ogg", "opus", "m4a", "aac"].indexOf(e) >= 0) return "file-audio"
        if (["zip", "tar", "gz", "xz", "zst", "bz2", "7z", "rar", "deb", "rpm", "iso"].indexOf(e) >= 0) return "file-archive"
        if (["rs", "js", "ts", "py", "c", "h", "cpp", "go", "java", "sh", "qml", "json", "toml", "yaml", "yml", "html", "css"].indexOf(e) >= 0) return "file-code"
        if (["txt", "md", "log", "conf", "ini"].indexOf(e) >= 0) return "file-text"
        return "file"
    }
    readonly property color fill: tile.lifted ? Qt.lighter(Theme.tile, 1.12) : Theme.tile
    readonly property real fold: Math.round(tile.width * 0.24)

    implicitWidth: 36
    implicitHeight: 36

    // ---- A folder ----
    Item {
        anchors.centerIn: parent
        visible: tile.folder
        width: tile.width
        height: tile.height * 0.78
        Rectangle {
            x: 0
            y: 0
            width: parent.width * 0.42
            height: parent.height * 0.3
            radius: 3
            color: tile.fill
        }
        Rectangle {
            y: parent.height * 0.14
            width: parent.width
            height: parent.height * 0.86
            radius: 4
            color: tile.fill
        }
        Rectangle {
            x: parent.width * 0.22
            y: parent.height * 0.6
            width: parent.width * 0.22
            height: 2
            radius: 1
            color: Theme.tileInk
            opacity: 0.55
        }
    }

    // ---- A file: the tile with its corner folded ----
    Item {
        anchors.fill: parent
        visible: !tile.folder && thumb.status !== Image.Ready

        Rectangle {
            anchors.fill: parent
            anchors.rightMargin: tile.fold
            radius: 5
            color: tile.fill
        }
        Rectangle {
            anchors.fill: parent
            anchors.topMargin: tile.fold
            radius: 5
            color: tile.fill
        }
        // The fold itself: a darker corner triangle.
        Canvas {
            x: tile.width - tile.fold
            y: 0
            width: tile.fold
            height: tile.fold
            onPaint: {
                const ctx = getContext("2d")
                ctx.reset()
                ctx.fillStyle = Qt.darker(tile.fill, 1.35)
                ctx.beginPath()
                ctx.moveTo(0, 0)
                ctx.lineTo(0, height)
                ctx.lineTo(width, height)
                ctx.closePath()
                ctx.fill()
            }
            Component.onCompleted: requestPaint()
        }

        Text {
            anchors.centerIn: parent
            visible: tile.letter.length > 0
            text: tile.letter
            color: Theme.tileInk
            font.family: Theme.family
            font.pixelSize: tile.letter.length > 1 ? tile.width * 0.24 : tile.width * 0.42
            font.weight: Font.Bold
        }
        Icon {
            anchors.centerIn: parent
            visible: tile.letter.length === 0
            name: tile.glyph
            size: tile.width * 0.52
            tint: Theme.tileInk
        }
    }

    // ---- A thumbnail, when the system has one ----
    Image {
        id: thumb
        anchors.fill: parent
        visible: !tile.folder && status === Image.Ready
        source: tile.folder ? "" : tile.thumbnail
        fillMode: Image.PreserveAspectCrop
        sourceSize: Qt.size(Math.max(2, tile.width * 2), Math.max(2, tile.height * 2))
        asynchronous: true
        smooth: true
        mipmap: true
    }
}
