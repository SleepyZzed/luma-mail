import QtQuick
HtmlSurface {
    id:root
    property string page:""
    property bool paused:false
    property var masks:[]
    property real uiScale:1
    property real revealProgress:1
    readonly property bool viewReady:contentReady
    readonly property string viewError:error
    document:page;blocked:paused;occluders:masks;zoom:uiScale
    background:Theme.window;reveal:revealProgress
    active:Mail.readerOpen&&Mail.windowVisible
    implicitHeight:contentHeight
    onLinkActivated:url=>Mail.openLink(url)
    Text{textFormat:Text.PlainText;anchors.fill:parent;text:root.error;visible:text!=="";wrapMode:Text.Wrap;color:Theme.muted;font.pixelSize:Theme.unit(12)}
}
