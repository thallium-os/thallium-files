import QtQuick

// Running copies, moves and deletes, floating at the foot of the file area.
// The window shows it only while something is actually running. Every
// figure comes from the bridge's task rows; `jobs` holds one
// { id, title, progress, done, total, rate } per active task.
Item {
    id: card
    property var jobs: []
    property bool paused: false
    property bool open: true
    property var format: function (bytes) { return String(bytes) }
    signal pauseToggled
    signal cancelled(var id)

    readonly property real rate: {
        let sum = 0
        for (let i = 0; i < card.jobs.length; ++i)
            sum += card.jobs[i].rate
        return sum
    }
    readonly property real remaining: {
        let left = 0
        for (let i = 0; i < card.jobs.length; ++i) {
            const job = card.jobs[i]
            if (job.total < 0)
                return -1
            left += Math.max(0, job.total - job.done)
        }
        return left
    }

    function timeLeft(bytes, rate) {
        if (bytes < 0 || rate <= 0)
            return ""
        const seconds = Math.ceil(bytes / rate)
        if (seconds < 90)
            return seconds + " s left"
        const minutes = Math.round(seconds / 60)
        if (minutes < 90)
            return minutes + " min left"
        return Math.round(minutes / 60) + " h left"
    }
    function speed(rate) { return rate > 0 ? card.format(rate) + "/s" : "" }
    function joined(parts) { return parts.filter(function (p) { return p.length > 0 }).join(" · ") }

    implicitHeight: content.implicitHeight + 34

    ChamferPanel { anchors.fill: parent }

    Column {
        id: content
        x: 18
        y: 16
        width: parent.width - 36
        spacing: 14

        Item {
            width: parent.width
            height: 22
            HudHeader {
                anchors.left: parent.left
                anchors.right: toggle.left
                anchors.rightMargin: 10
                title: "Transfers"
                sub: card.jobs.length + " live"
                trailing: card.paused ? "RESUME" : "PAUSE"
                trailingColor: card.paused ? Theme.cGreen : Theme.cFg
                onTrailingClicked: card.pauseToggled()
            }
            Text {
                id: toggle
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: card.open ? "–" : "+"
                color: toggleMouse.containsMouse ? Theme.cGreen : Theme.cFg
                font.family: Theme.fontMono
                font.pixelSize: 12
                MouseArea {
                    id: toggleMouse
                    anchors.fill: parent
                    anchors.margins: -6
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: card.open = !card.open
                }
            }
        }

        Text {
            width: parent.width
            elide: Text.ElideRight
            text: (card.paused ? "All paused"
                  : card.joined([card.timeLeft(card.remaining, card.rate)
                                 ? "about " + card.timeLeft(card.remaining, card.rate) : "",
                                 card.speed(card.rate)]) || "Starting").toUpperCase()
            color: Theme.cMuted
            font.family: Theme.fontMono
            font.pixelSize: 11
            font.letterSpacing: 1
        }

        Repeater {
            model: card.open ? card.jobs : []
            delegate: Column {
                id: job
                required property var modelData
                width: content.width
                spacing: 8

                Rectangle { width: parent.width; height: 1; color: Theme.cDead }
                Item { width: 1; height: 3 }

                Item {
                    width: parent.width
                    height: 24
                    Text {
                        anchors.left: parent.left
                        anchors.right: pct.left
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        elide: Text.ElideMiddle
                        text: job.modelData.title
                        color: Theme.cFg
                        font.family: Theme.fontHuman
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                    }
                    Text {
                        id: pct
                        anchors.right: cancel.left
                        anchors.rightMargin: 10
                        anchors.verticalCenter: parent.verticalCenter
                        text: Math.round(job.modelData.progress) + "%"
                        color: card.paused ? Theme.cMuted : Theme.cGreen
                        font.family: Theme.fontBrand
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                    }
                    IconButton {
                        id: cancel
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        implicitWidth: 24
                        implicitHeight: 24
                        icon: "x"
                        iconSize: 12
                        hint: "Cancel"
                        onClicked: card.cancelled(job.modelData.id)
                    }
                }
                SegmentBar {
                    x: 3
                    width: parent.width - 6
                    height: 8
                    count: 28
                    share: job.modelData.progress / 100
                    opacity: card.paused ? 0.5 : 1
                }
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: card.joined([
                        job.modelData.total >= 0 ? card.format(job.modelData.done) + " / " + card.format(job.modelData.total) : "",
                        card.paused ? "paused" : card.speed(job.modelData.rate),
                        card.paused ? "" : card.timeLeft(job.modelData.total < 0 ? -1 : job.modelData.total - job.modelData.done, job.modelData.rate)
                    ]).toUpperCase()
                    color: Theme.cMuted
                    font.family: Theme.fontMono
                    font.pixelSize: 10
                    font.letterSpacing: 1
                }
            }
        }
    }
}
