import QtQuick
import QtQuick.Controls

// The reference's dialog: icon and title with a close button, a
// cross-ended rule, the content, and a darker footer holding the buttons.
Popup {
    id: sheet

    property string title: ""
    property string icon: "folder"
    default property alias content: body.data
    property alias buttons: footRow.data
    property bool showFooter: true

    anchors.centerIn: Overlay.overlay
    width: 460
    modal: true
    focus: true
    padding: 0

    Overlay.modal: Rectangle { color: "#A0000000" }

    background: Rectangle {
        color: Theme.canvas
        radius: Theme.radius
        border.width: 1
        border.color: Theme.outlineSoft
    }

    contentItem: Column {
        width: sheet.width

        Item {
            width: parent.width
            height: 62
            Icon {
                id: headIcon
                x: 26
                anchors.verticalCenter: parent.verticalCenter
                name: sheet.icon
                size: 18
                shade: 0.75
            }
            Text {
                anchors.left: headIcon.right
                anchors.leftMargin: 12
                anchors.right: close.left
                anchors.rightMargin: 12
                anchors.verticalCenter: parent.verticalCenter
                text: sheet.title
                elide: Text.ElideRight
                color: Theme.ink
                font.family: Theme.family
                font.pixelSize: Theme.textLarge
                font.weight: Font.Medium
            }
            IconButton {
                id: close
                anchors.right: parent.right
                anchors.rightMargin: 18
                anchors.verticalCenter: parent.verticalCenter
                implicitWidth: 30
                implicitHeight: 30
                color: Theme.control
                icon: "x"
                iconSize: 15
                onClicked: sheet.close()
            }
        }

        CrossRule {
            x: 22
            width: parent.width - 44
        }

        Column {
            id: body
            x: 28
            width: parent.width - 56
            topPadding: 18
            bottomPadding: 22
            spacing: 12
        }

        Item {
            visible: sheet.showFooter
            width: parent.width
            height: sheet.showFooter ? 64 : 0
            // Footer with the sheet's bottom corners and a square top.
            Rectangle {
                anchors.fill: parent
                anchors.margins: 1
                radius: Theme.radius
                color: Theme.surface
            }
            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 1
                height: Theme.radius
                color: Theme.surface
            }
            Row {
                id: footRow
                anchors.right: parent.right
                anchors.rightMargin: 22
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
            }
        }
    }
}
