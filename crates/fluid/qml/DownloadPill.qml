import QtQuick
import QtQuick.Controls.Basic
Rectangle {
    id:root;z:180
    visible:AttachmentViewer.downloadMessage!==""
    width:Theme.unit(330);height:Math.max(0,extent.value);radius:Theme.unit(16);color:Theme.surface;border.width:1;border.color:Theme.outline;clip:true
    Motion{id:extent;target:root.visible?Theme.unit(76):0;enabled:Theme.motion}
    Text{textFormat:Text.PlainText;x:Theme.unit(15);y:Theme.unit(13);width:parent.width-Theme.unit(58);text:AttachmentViewer.downloadMessage;elide:Text.ElideMiddle;color:Theme.text;font.pixelSize:Theme.unit(11)}
    FluidButton{anchors.right:parent.right;anchors.top:parent.top;glyph:"close";tip:AttachmentViewer.downloading?"Cancel download":"Dismiss";enabled:!AttachmentViewer.cancelling;onClicked:{if(AttachmentViewer.downloading)AttachmentViewer.cancelDownload();else AttachmentViewer.dismissDownload()}}
    Rectangle{x:Theme.unit(15);y:Theme.unit(46);width:parent.width-2*x;height:Theme.unit(3);radius:height/2;color:Theme.outline
        Rectangle{height:parent.height;width:parent.width*Math.max(0,Math.min(1,progress.value));radius:height/2;color:Theme.accent}
        Motion{id:progress;target:AttachmentViewer.downloadProgress;enabled:Theme.motion;epsilon:.001}
    }
    Text{textFormat:Text.PlainText;x:Theme.unit(15);y:Theme.unit(55);text:AttachmentViewer.downloading?(AttachmentViewer.cancelling?"Finishing cleanup":"Downloads go only to your chosen folder"):"No attachment copy is retained by Luma";font.pixelSize:Theme.unit(9);color:Theme.muted}
}
