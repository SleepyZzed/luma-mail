import QtQuick
Item {
    id:root
    property bool motion:Mail.windowVisible&&!(Mail.settings.reduced_motion||false)
    property color target:"transparent"
    readonly property color value:Qt.rgba(Math.max(0,Math.min(1,red.value)),Math.max(0,Math.min(1,green.value)),Math.max(0,Math.min(1,blue.value)),Math.max(0,Math.min(1,alpha.value)))
    Motion{id:red;target:root.target.r;epsilon:.001;enabled:root.motion}
    Motion{id:green;target:root.target.g;epsilon:.001;enabled:root.motion}
    Motion{id:blue;target:root.target.b;epsilon:.001;enabled:root.motion}
    Motion{id:alpha;target:root.target.a;epsilon:.001;enabled:root.motion}
}
