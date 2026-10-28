import QtQuick

Item {
    id: root
    objectName: "rowMorph"
    property var row: ({})
    property bool active: false
    property bool opening: true
    property rect rowRect: Qt.rect(0, 0, 0, 0)
    property var destination: ({bounds: Qt.rect(0, 0, 0, 0), avatar: Qt.rect(0, 0, 38, 38), sender: Qt.rect(52, 0, 200, 18)})
    property var source: ({avatar: Qt.rect(16, 12, 32, 32), sender: Qt.rect(60, 10, 200, 16), subject: Qt.rect(60, 27, 200, 16), snippet: Qt.rect(60, 44, 200, 14)})
    readonly property real progress: Math.max(0, Math.min(1, shape.value))
    readonly property bool moving: horizontal.running || vertical.running || breadth.running || depth.running || shape.running
    readonly property rect goal: opening ? destination.bounds : rowRect
    visible: active
    clip: true
    z: 50
    x: horizontal.value
    y: vertical.value
    width: Math.max(0, breadth.value)
    height: Math.max(0, depth.value)

    Motion { id: horizontal; target: root.goal.x; enabled: Theme.motion }
    Motion { id: vertical; target: root.goal.y; enabled: Theme.motion }
    Motion { id: breadth; target: root.goal.width; enabled: Theme.motion }
    Motion { id: depth; target: root.goal.height; enabled: Theme.motion }
    Motion { id: shape; target: root.opening ? 1 : 0; epsilon: .001; enabled: Theme.motion }
    onMovingChanged: if (!moving && active) Qt.callLater(root.settle)

    function mix(a, b) { return a + (b - a) * progress }
    function place(item, bounds, target, enter) {
        if (!Theme.motion) { active = false; return }
        const wasActive = active
        row = item
        if (!wasActive && item._projection && item._projection.avatar) source = item._projection
        rowRect = bounds
        destination = target
        opening = enter
        if (!wasActive) {
            const initial = enter ? bounds : target.bounds
            horizontal.snap(initial.x)
            vertical.snap(initial.y)
            breadth.snap(initial.width)
            depth.snap(initial.height)
            shape.snap(enter ? 0 : 1)
            horizontal.resume(); vertical.resume(); breadth.resume(); depth.resume(); shape.resume()
        }
        active = true
    }
    function settle() {
        if (!moving && Math.abs(x - goal.x) < 1 && Math.abs(y - goal.y) < 1
                && Math.abs(width - goal.width) < 1 && Math.abs(height - goal.height) < 1)
            active = false
    }
    Rectangle {
        anchors.fill: parent
        radius: Theme.unit(root.mix(17, 23))
        color: Theme.selected
        opacity: 1 - root.progress
    }
    Avatar {
        objectName:"projectedAvatar"
        account:root.row.key?.account||"";active:root.active
        x: root.mix(root.source.avatar.x, root.destination.avatar.x)
        y: root.mix(root.source.avatar.y, root.destination.avatar.y)
        width: root.mix(root.source.avatar.width, root.destination.avatar.width)
        height: width
        sender: root.row.sender || ""
        photo: root.row.photo || ""
    }
    Text {textFormat:Text.PlainText;
        x: root.mix(root.source.sender.x, root.destination.sender.x)
        y: root.mix(root.source.sender.y, root.destination.sender.y)
        width: Math.max(0, parent.width - x - Theme.unit(20))
        text: (root.row.sender || "").replace(/<.*>/g, "").trim()
        font.pixelSize: Theme.unit(12)
        font.weight: Font.DemiBold
        color: Theme.text
        elide: Text.ElideRight
    }
    Text {textFormat:Text.PlainText;
        x: root.mix(root.source.subject.x, 0)
        y: root.mix(root.source.subject.y, 0)
        width: Math.max(0, parent.width - x - Theme.unit(20))
        text: root.row.subject || "(No subject)"
        font.pixelSize: Theme.unit(root.mix(12, 25))
        lineHeight:root.mix(1,1.2)
        font.weight: Font.DemiBold
        color: Theme.text
        wrapMode: root.progress > .65 ? Text.Wrap : Text.NoWrap
        elide: Text.ElideRight
    }
    Text {textFormat:Text.PlainText;
        x: root.source.snippet.x
        y: root.source.snippet.y
        width: Math.max(0, parent.width - x - Theme.unit(20))
        text: root.row.snippet || ""
        font.pixelSize: Theme.unit(10)
        color: Theme.faint
        elide: Text.ElideRight
        opacity: (root.source.snippetVisible===false?0:1)*(1-root.progress)
    }
}
