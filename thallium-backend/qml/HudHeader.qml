import QtQuick

// A section title as the Store rules it: a green tick, the title in spaced
// mono capitals, an optional muted sub-label, then a hairline that runs to
// the trailing text (or the edge).
Item {
    id: hud
    property string title: ""
    property string sub: ""
    property string trailing: ""
    property color trailingColor: Theme.cFg
    property int titleSize: 13
    // A fold control at the far right: when `foldable`, ▾ while open and ▸
    // while folded; the owner flips `folded` on `foldToggled`.
    property bool foldable: false
    property bool folded: false
    signal trailingClicked
    signal foldToggled

    implicitHeight: 22

    Rectangle {
        id: tick
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Theme.tickW
        height: 15
        color: Theme.cGreen
    }
    Text {
        id: titleText
        anchors.left: tick.right
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        text: hud.title.toUpperCase()
        color: Theme.cFg
        font.family: Theme.fontMono
        font.pixelSize: hud.titleSize
        font.bold: true
        font.letterSpacing: 2.6
    }
    Text {
        id: subText
        anchors.left: titleText.right
        anchors.leftMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        visible: hud.sub.length > 0
        text: hud.sub
        color: Theme.cMuted
        font.family: Theme.fontMono
        font.pixelSize: 11
        font.letterSpacing: 1
    }
    Rectangle {
        anchors.left: subText.visible ? subText.right : titleText.right
        anchors.leftMargin: 12
        anchors.right: trailingText.visible ? trailingText.left : foldButton.visible ? foldButton.left : parent.right
        anchors.rightMargin: trailingText.visible || foldButton.visible ? 12 : 0
        anchors.verticalCenter: parent.verticalCenter
        height: 1
        color: Theme.cLine
    }
    Text {
        id: trailingText
        anchors.right: foldButton.visible ? foldButton.left : parent.right
        anchors.rightMargin: foldButton.visible ? 10 : 0
        anchors.verticalCenter: parent.verticalCenter
        visible: hud.trailing.length > 0
        text: hud.trailing
        color: trailingMouse.containsMouse ? Theme.cGreen : hud.trailingColor
        font.family: Theme.fontMono
        font.pixelSize: 11
        font.letterSpacing: 2
        MouseArea {
            id: trailingMouse
            anchors.fill: parent
            anchors.margins: -4
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: hud.trailingClicked()
        }
    }
    Rectangle {
        id: foldButton
        visible: hud.foldable
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: 22
        height: 22
        color: foldMouse.containsMouse ? Theme.cDim : "transparent"
        border.width: 1
        border.color: foldMouse.containsMouse ? Theme.cGreen : Theme.cDead
        Text {
            anchors.centerIn: parent
            text: hud.folded ? "▸" : "▾"
            color: foldMouse.containsMouse || !hud.folded ? Theme.cGreen : Theme.cMuted
            font.family: Theme.fontMono
            font.pixelSize: 11
        }
        MouseArea {
            id: foldMouse
            anchors.fill: parent
            anchors.margins: -4
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: hud.foldToggled()
        }
    }
}
