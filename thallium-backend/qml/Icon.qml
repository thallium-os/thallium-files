import QtQuick
import QtQuick.Effects

// A Lucide outline icon from icons/. Drawn white and shaded by opacity, so
// it takes the lightness of the ink around it, or recoloured with `tint`
// where it has to sit dark on a light tile.
Item {
    id: icon
    property string name: "file"
    property real size: 18
    property real shade: 0.62
    property color tint: "transparent"
    readonly property bool tinted: icon.tint.a > 0

    width: size
    height: size

    Image {
        id: image
        anchors.fill: parent
        sourceSize: Qt.size(icon.size * 2, icon.size * 2)
        source: "icons/" + icon.name + ".svg"
        opacity: icon.tinted ? 1 : icon.shade
        visible: !icon.tinted
        smooth: true
    }

    MultiEffect {
        anchors.fill: parent
        source: image
        visible: icon.tinted
        brightness: 1.0
        colorization: 1.0
        colorizationColor: icon.tint
    }
}
