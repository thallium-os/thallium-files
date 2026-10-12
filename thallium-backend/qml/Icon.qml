import QtQuick

// A Lucide outline icon from icons/, drawn white and shaded by opacity, so
// it takes the lightness of the ink around it.
Image {
    id: icon
    property string name: "file"
    property real size: 18
    property real shade: 0.62
    width: size
    height: size
    sourceSize: Qt.size(size * 2, size * 2)
    source: "icons/" + name + ".svg"
    opacity: shade
    smooth: true
}
