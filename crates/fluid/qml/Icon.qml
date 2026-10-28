import QtQuick
Item {
    id: root
    property string name: "inbox"
    property color color: Theme.text
    readonly property bool software:typeof softwareGraphics!=="undefined"&&softwareGraphics
    implicitWidth: Theme.unit(19); implicitHeight: implicitWidth
    Image {id: glyph;anchors.fill: parent;source: root.name?"qrc:/icons/luma-"+(root.name==="label"?"tag":root.name)+"-symbolic.svg":"";sourceSize: Qt.size(width*Screen.devicePixelRatio,height*Screen.devicePixelRatio);visible:false}
    ShaderEffect {anchors.fill: glyph;visible:!root.software;property var source:glyph;property color tint:root.color;fragmentShader:"qrc:/shaders/icon.frag.qsb"}
    Image{anchors.fill:parent;visible:root.software;cache:false;source:root.software&&root.name?"image://software-icon/"+root.color.toString().replace("#","")+"/"+(root.name==="label"?"tag":root.name):"";sourceSize:Qt.size(width*Screen.devicePixelRatio,height*Screen.devicePixelRatio)}
}
