import QtQuick

// A text tab in spaced mono capitals; the active one is lettered in the
// body colour with a 2px accent underline at its foot.
Item {
    id: tab
    property string text: ""
    property bool active: false
    property int underlineInset: 0
    property real textSize: 11.5
    signal clicked

    implicitWidth: label.implicitWidth + 24
    implicitHeight: 36

    Text {
        id: label
        anchors.centerIn: parent
        text: tab.text.toUpperCase()
        color: tab.active ? Theme.cFg : mouse.containsMouse ? Theme.cFg : Theme.cMuted
        font.family: Theme.fontMono
        font.pixelSize: Math.round(tab.textSize)
        font.letterSpacing: 3
        font.bold: tab.active
    }
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.leftMargin: tab.underlineInset
        anchors.rightMargin: tab.underlineInset
        height: 2
        color: Theme.cGreen
        visible: tab.active
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: tab.clicked()
    }
}
