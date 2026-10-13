import QtQuick

// A square check, light when ticked, with an optional label.
Item {
    id: check
    property bool checked: false
    property string text: ""
    signal toggled(bool checked)

    implicitWidth: row.implicitWidth
    implicitHeight: 22
    opacity: check.enabled ? 1 : 0.4

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: 9
        Rectangle {
            width: 18
            height: 18
            radius: 4
            color: check.checked ? Theme.inkSoft : (mouse.containsMouse ? Theme.surfaceHigh : Theme.surface)
            border.width: 1
            border.color: check.checked ? Theme.inkSoft : Theme.outline
            Icon {
                anchors.centerIn: parent
                visible: check.checked
                name: "check"
                size: 13
                tint: Theme.tileInk
            }
        }
        Text {
            visible: check.text.length > 0
            anchors.verticalCenter: parent.verticalCenter
            text: check.text
            color: Theme.inkSoft
            font.family: Theme.family
            font.pixelSize: Theme.text
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: {
            check.checked = !check.checked
            check.toggled(check.checked)
        }
    }
}
