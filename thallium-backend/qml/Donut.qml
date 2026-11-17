import QtQuick
import QtQuick.Shapes

// A ring split into segments by value, with a figure in the middle.
// `segments` is a list of { value, color }; empty ones are skipped.
Item {
    id: donut

    property var segments: []
    property string figure: ""
    property string caption: ""
    property real thickness: 14

    implicitWidth: 128
    implicitHeight: 128

    readonly property real total: {
        let sum = 0
        for (let i = 0; i < donut.segments.length; ++i)
            sum += Math.max(0, donut.segments[i].value)
        return sum
    }
    readonly property real radius: (Math.min(width, height) - donut.thickness) / 2
    // Degrees left blank between neighbouring segments.
    readonly property real gap: donut.arcs.length > 1 ? 2 : 0

    readonly property var arcs: {
        const out = []
        if (donut.total <= 0)
            return out
        let at = -90
        for (let i = 0; i < donut.segments.length; ++i) {
            const value = Math.max(0, donut.segments[i].value)
            if (value <= 0)
                continue
            const sweep = value / donut.total * 360
            out.push({ start: at, sweep: sweep, color: donut.segments[i].color })
            at += sweep
        }
        return out
    }

    // The empty track, so a ring with nothing in it still reads as a ring.
    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            fillColor: "transparent"
            strokeColor: Theme.control
            strokeWidth: donut.thickness
            capStyle: ShapePath.FlatCap
            PathAngleArc {
                centerX: donut.width / 2; centerY: donut.height / 2
                radiusX: donut.radius; radiusY: donut.radius
                startAngle: 0; sweepAngle: 360
            }
        }
    }

    Repeater {
        model: donut.arcs
        delegate: Shape {
            id: arc
            required property var modelData
            anchors.fill: parent
            preferredRendererType: Shape.CurveRenderer
            ShapePath {
                fillColor: "transparent"
                strokeColor: arc.modelData.color
                strokeWidth: donut.thickness
                capStyle: ShapePath.FlatCap
                PathAngleArc {
                    centerX: donut.width / 2; centerY: donut.height / 2
                    radiusX: donut.radius; radiusY: donut.radius
                    startAngle: arc.modelData.start + donut.gap / 2
                    sweepAngle: Math.max(0.5, arc.modelData.sweep - donut.gap)
                }
            }
        }
    }

    Column {
        anchors.centerIn: parent
        spacing: 2
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: donut.figure
            color: Theme.ink
            font.family: Theme.mono
            font.pixelSize: 18
            font.weight: Font.DemiBold
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: donut.caption.length > 0
            text: donut.caption
            color: Theme.inkDim
            font.family: Theme.mono
            font.pixelSize: 9
            font.letterSpacing: Theme.letterSpace
        }
    }
}
