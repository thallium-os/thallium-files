import QtQuick

// The hairline the window is ruled with. On Thallium 81 it is a plain 1px
// rule; the other palettes keep the small cross at each end.
Item {
  id: rule
  property color color: Theme.t81 ? Theme.cDead : Theme.line
  property color crossColor: Theme.cross
  implicitHeight: 9

  Rectangle {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Theme.t81 ? 0 : 4
    anchors.rightMargin: Theme.t81 ? 0 : 4
    anchors.verticalCenter: parent.verticalCenter
    height: 1
    color: rule.color
  }

  Repeater {
    model: Theme.t81 ? 0 : 2
    delegate: Item {
      required property int index
      width: 9
      height: 9
      x: index === 0 ? 0 : rule.width - width
      y: (rule.height - height) / 2
      Rectangle { anchors.centerIn: parent; width: 9; height: 1; color: rule.crossColor }
      Rectangle { anchors.centerIn: parent; width: 1; height: 9; color: rule.crossColor }
    }
  }
}
