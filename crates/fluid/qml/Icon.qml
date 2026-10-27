import QtQuick
Item {
    id: root
    property string name: "inbox"
    property color color: Theme.text
    implicitWidth: Theme.unit(19); implicitHeight: implicitWidth
    Image {id: glyph;anchors.fill: parent;source: root.name?"qrc:/icons/luma-"+root.name+"-symbolic.svg":"";sourceSize: Qt.size(width*Screen.devicePixelRatio,height*Screen.devicePixelRatio);visible:false}
    ShaderEffect {anchors.fill: glyph;property var source:glyph;property color tint:root.color;fragmentShader:"qrc:/shaders/icon.frag.qsb"}
}
