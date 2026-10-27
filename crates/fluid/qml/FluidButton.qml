import QtQuick
import QtQuick.Controls
AbstractButton {
    id: control
    property string glyph: ""
    property bool primary: false
    property bool quiet: true
    property color tint: primary?Theme.accentText:Theme.text
    property string tip: ""
    hoverEnabled:true;focusPolicy:Qt.StrongFocus
    implicitWidth: Math.max(Theme.unit(34),contentRow.implicitWidth+Theme.unit(text?28:16))
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
    contentItem: Row {
        id:contentRow;spacing:Theme.unit(9);anchors.centerIn:parent
        Icon {name:control.glyph;color:control.tint;visible:control.glyph!=="";anchors.verticalCenter:parent.verticalCenter}
        Text {text:control.text;color:control.enabled?control.tint:Theme.faint;font.family:Theme.family;font.pixelSize:Theme.unit(13);font.weight:Font.DemiBold;anchors.verticalCenter:parent.verticalCenter;visible:text!==""}
    }
    ToolTip.visible:hovered&&tip!=="";ToolTip.text:tip;ToolTip.delay:550
    Accessible.name:tip||text
}
