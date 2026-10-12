import QtQuick
import QtQuick.Controls

// A square, borderless icon button. `on` fills and outlines it, for toggles.
Rectangle {
    id: button
    property string icon: "file"
    property string hint: ""
    property bool on: false
    property real iconSize: 18
    signal clicked

    implicitWidth: Theme.controlHeight
    implicitHeight: Theme.controlHeight
    radius: Theme.radius
    color: button.on ? Theme.selected : (mouse.containsMouse ? Theme.surfaceHigh : "transparent")
    border.width: button.on ? 1 : 0
    border.color: Theme.outline
    opacity: button.enabled ? 1 : 0.35

    Icon {
        anchors.centerIn: parent
        name: button.icon
        size: button.iconSize
        shade: button.on ? 1 : (mouse.containsMouse ? 0.95 : 0.7)
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: button.clicked()
    }

    ToolTip.visible: button.hint.length > 0 && mouse.containsMouse
    ToolTip.delay: 600
    ToolTip.text: button.hint
}
