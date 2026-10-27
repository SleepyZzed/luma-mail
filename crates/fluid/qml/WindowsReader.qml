import QtQuick
HtmlSurface {
    id:root
    property string page:""
    property bool paused:false
    property var masks:[]
    property real uiScale:1
    document:page;blocked:paused;occluders:masks;zoom:uiScale
    implicitHeight:contentHeight
    onLinkActivated:url=>Mail.openLink(url)
    Text{textFormat:Text.PlainText;anchors.fill:parent;text:root.error;visible:text!=="";wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
}
