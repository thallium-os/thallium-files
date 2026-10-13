import QtQuick

// The narrow column of sections down the left edge. The current section is
// filled and outlined all the way round; nothing is marked on one side only.
Rectangle {
    id: rail

    property string current: "files"
    // Without rclone there is nothing behind Network, so it is not offered.
    property bool network: true
    signal chosen(string key)
    signal moreRequested(Item anchor)

    readonly property var sections: [
        { key: "home", icon: "house", label: "HOME" },
        { key: "files", icon: "folder", label: "FILES" },
        { key: "recent", icon: "clock", label: "RECENT" },
        { key: "trash", icon: "trash-2", label: "TRASH" },
        { key: "network", icon: "globe", label: "NETWORK" },
        { key: "settings", icon: "settings", label: "SETTINGS" }
    ]

    width: Theme.railWidth
    color: Theme.surface

    // Thallium's element symbol stands where a product mark would.
    Rectangle {
        id: mark
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.top: parent.top
        anchors.topMargin: 22
        width: 34
        height: 34
        radius: Theme.radiusSmall
        color: "transparent"
        border.width: 1
        border.color: Theme.outline

        Text {
            anchors.centerIn: parent
            text: "Tl"
            color: Theme.ink
            font.family: Theme.family
            font.pixelSize: 15
            font.weight: Font.DemiBold
        }
    }

    Column {
        anchors.top: mark.bottom
        anchors.topMargin: 26
        anchors.horizontalCenter: parent.horizontalCenter
        spacing: 6

        Repeater {
            model: rail.sections.filter(function (section) { return section.key !== "network" || rail.network })
            delegate: Rectangle {
                id: item
                required property var modelData
                readonly property bool active: rail.current === item.modelData.key

                width: 66
                height: 64
                radius: Theme.radius
                color: item.active ? Theme.selected : (itemMouse.containsMouse ? Theme.surfaceHigh : "transparent")
                border.width: item.active ? 1 : 0
                border.color: Theme.outline

                Column {
                    anchors.centerIn: parent
                    spacing: 7

                    Icon {
                        anchors.horizontalCenter: parent.horizontalCenter
                        name: item.modelData.icon
                        size: 20
                        shade: item.active ? 1 : (itemMouse.containsMouse ? 0.8 : 0.5)
                    }

                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: item.modelData.label
                        color: item.active ? Theme.ink : Theme.inkFaint
                        font.family: Theme.family
                        font.pixelSize: 9
                        font.weight: Font.Medium
                        font.letterSpacing: 0.8
                    }
                }

                MouseArea {
                    id: itemMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: rail.chosen(item.modelData.key)
                }
            }
        }
    }

    // The overflow menu, at the foot like the reference's account card.
    Rectangle {
        id: more
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 18
        width: 58
        height: 46
        radius: Theme.radius
        color: moreMouse.containsMouse ? Theme.surfaceHigh : Theme.control
        border.width: 1
        border.color: Theme.outlineSoft

        Icon {
            anchors.centerIn: parent
            name: "ellipsis"
            size: 20
            shade: moreMouse.containsMouse ? 0.95 : 0.7
        }

        MouseArea {
            id: moreMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: rail.moreRequested(more)
        }
    }
}
