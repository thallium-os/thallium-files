import QtQuick

// The hairline the whole design is ruled with: a faint line that ends in a
// small cross at each end.
Item {
    id: rule
    property color color: Theme.line
    property color crossColor: Theme.cross
    implicitHeight: 9

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 4
        anchors.rightMargin: 4
        anchors.verticalCenter: parent.verticalCenter
        height: 1
        color: rule.color
    }

    Repeater {
        model: 2
        delegate: Item {
            required property int index
            width: 9
            height: 9
            x: index === 0 ? 0 : rule.width - width
            anchors.verticalCenter: parent.verticalCenter
            Rectangle { anchors.centerIn: parent; width: 9; height: 1; color: rule.crossColor }
            Rectangle { anchors.centerIn: parent; width: 1; height: 9; color: rule.crossColor }
        }
    }
}
