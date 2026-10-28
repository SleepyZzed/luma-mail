import QtQuick
import QtQuick.Controls.Basic
AbstractButton {
    id: control
    property string glyph: ""
    property bool primary: false
    property bool quiet: true
    property color tint: primary?Theme.accentText:Theme.text
    property string tip: ""
    property real labelWidth:-1
    property bool alignLeading:false
    leftPadding:Theme.unit(text?14:8);rightPadding:leftPadding
    topPadding:Theme.unit(6);bottomPadding:topPadding
    hoverEnabled:true;focusPolicy:Qt.StrongFocus
    implicitWidth: Math.max(Theme.unit(34),(glyph?Theme.unit(19):0)+(text?buttonLabel.implicitWidth:0)+(glyph&&text?Theme.unit(9):0)+leftPadding+rightPadding)
    implicitHeight: Theme.unit(34)
    scale:press.value
    Motion{id:press;target:control.down?.96:1;stiffness:520;damping:40;epsilon:.001;enabled:Theme.motion}
    background: Rectangle {
        radius:Theme.unit(control.primary?15:10)
        color:control.primary?Theme.accent:Theme.hover
        opacity:Math.max(0,Math.min(1,highlight.value))
        Motion{id:highlight;target:control.primary?1:control.hovered||control.visualFocus?.95:control.quiet?0:.5;epsilon:.002;enabled:Theme.motion}
        border.width:control.visualFocus?1:0;border.color:Theme.accent
    }
    contentItem: Item {
        implicitWidth:contentRow.implicitWidth;implicitHeight:contentRow.implicitHeight
        Row {
            id:contentRow;objectName:"buttonContents";spacing:Theme.unit(9);x:control.alignLeading?0:(parent.width-width)/2;anchors.verticalCenter:parent.verticalCenter
            Icon {name:control.glyph;color:control.enabled?control.tint:Theme.faint;visible:control.glyph!=="";anchors.verticalCenter:parent.verticalCenter}
            Text {id:buttonLabel;textFormat:Text.PlainText;text:control.text;width:Math.max(0,Math.min(implicitWidth,control.labelWidth>=0?control.labelWidth:implicitWidth,control.availableWidth-(control.glyph?Theme.unit(28):0)));elide:Text.ElideRight;color:control.enabled?control.tint:Theme.faint;font.family:Theme.family;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold;anchors.verticalCenter:parent.verticalCenter;visible:text!==""}
        }
    }
    ToolTip.visible:hovered&&tip!=="";ToolTip.text:tip;ToolTip.delay:550
    Accessible.name:tip||text
}
