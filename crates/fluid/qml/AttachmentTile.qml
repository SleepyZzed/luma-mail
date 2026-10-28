import QtQuick
import QtQuick.Controls.Basic
import Luma.Fluid
Rectangle {
    id:root
    required property var attachment
    required property var owner
    property bool removable:false
    property bool ready:false
    property string thumbnailKey:""
    property string thumbnailSource:""
    readonly property bool active:visibilitySensor.viewportHeight>0&&Mail.windowVisible&&visible&&attachment.kind!=="drive"&&(owner.session!==undefined||Mail.settings.auto_preview_attachments!==false)
    signal removeRequested()
    width:Theme.unit(235);height:Theme.unit(66);radius:Theme.unit(13)
    color:hover.hovered?Theme.raised:Theme.surface;border.width:1;border.color:Theme.outline
    function parameters(){let value=Object.assign({},owner);if(attachment.part!==undefined)value.part=attachment.part;else value.attachment=attachment.id;return value}
    function preview(){if(attachment.kind==="drive"){Mail.openLink(attachment.url||"");return}const point=mapToItem(null,0,0);AttachmentViewer.open(parameters(),attachment.name,Qt.rect(point.x,point.y,width,height))}
    function subscribe(){
        if(!ready)return
        if(thumbnailKey)AttachmentViewer.releaseThumbnail(thumbnailKey)
        thumbnailKey="";thumbnailSource=""
        if(active){thumbnailKey=AttachmentViewer.watchThumbnail(parameters(),attachment.name,attachment.mime||"",attachment.size||0);thumbnailSource=AttachmentViewer.thumbnailSource(thumbnailKey)}
    }
    onActiveChanged:if(ready)Qt.callLater(subscribe)
    onOwnerChanged:if(ready)Qt.callLater(subscribe)
    onAttachmentChanged:if(ready)Qt.callLater(subscribe)
    Component.onCompleted:{ready=true;subscribe()}
    Component.onDestruction:{ready=false;if(thumbnailKey)AttachmentViewer.releaseThumbnail(thumbnailKey)}
    HtmlSurface{id:visibilitySensor;anchors.fill:parent}
    Connections{target:AttachmentViewer;function onThumbnailChanged(key){if(key===root.thumbnailKey)root.thumbnailSource=AttachmentViewer.thumbnailSource(key)}}
    HoverHandler{id:hover}
    Icon{x:Theme.unit(12);anchors.verticalCenter:parent.verticalCenter;name:"attachment";color:Theme.accent;width:Theme.unit(22);height:width;visible:thumbnail.status!==Image.Ready}
    Image{id:thumbnail;objectName:"attachmentThumbnail-"+root.thumbnailKey;x:Theme.unit(8);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(49);height:Theme.unit(49);source:root.thumbnailSource;sourceSize:Qt.size(128,96);cache:false;asynchronous:true;fillMode:Image.PreserveAspectFit;scale:thumbReveal.value}
    Motion{id:thumbReveal;target:thumbnail.status===Image.Ready?1:0;enabled:Theme.motion;epsilon:.002}
    Column{x:root.thumbnailSource?Theme.unit(66):Theme.unit(45);anchors.verticalCenter:parent.verticalCenter;width:parent.width-x-Theme.unit(37);spacing:Theme.unit(5)
        Text{textFormat:Text.PlainText;width:parent.width;text:root.attachment.name;color:Theme.text;elide:Text.ElideMiddle;font.pixelSize:Theme.unit(11);font.weight:Font.Medium}
        Text{textFormat:Text.PlainText;width:parent.width;text:root.attachment.kind==="drive"?"Drive link":(root.attachment.size>=1048576?(root.attachment.size/1048576).toFixed(1)+" MiB":Math.ceil(root.attachment.size/1024)+" KiB");color:Theme.muted;font.pixelSize:Theme.unit(9)}
    }
    MouseArea{anchors.fill:parent;acceptedButtons:Qt.LeftButton|Qt.RightButton;onClicked:event=>{if(event.button===Qt.RightButton)context.open();else root.preview()}}
    FluidButton{anchors.right:parent.right;anchors.verticalCenter:parent.verticalCenter;glyph:root.removable?"close":"download";tip:root.removable?"Remove attachment":"Save attachment to folder";visible:root.attachment.kind!=="drive"||root.removable;onClicked:{if(root.removable)root.removeRequested();else AttachmentViewer.save(root.parameters(),root.attachment.name)}}
    FluidMenu{id:context
        FluidMenuItem{text:root.attachment.kind==="drive"?"Open Drive link":"Preview";onTriggered:root.preview()}
        FluidMenuItem{text:"Save to folder…";visible:root.attachment.kind!=="drive";height:visible?implicitHeight:0;onTriggered:AttachmentViewer.save(root.parameters(),root.attachment.name)}
    }
}
