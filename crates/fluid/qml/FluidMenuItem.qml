import QtQuick
import QtQuick.Controls.Basic
MenuItem {
    id:control
    property string glyph:""
    implicitWidth:Theme.unit(270)
    implicitHeight:Theme.unit(36)
    leftPadding:Theme.unit(glyph?38:12);rightPadding:Theme.unit(subMenu?30:12)
    topPadding:Theme.unit(8);bottomPadding:topPadding
    hoverEnabled:true
    font.family:Theme.family;font.pixelSize:Theme.unit(12)
    contentItem:Text{textFormat:Text.PlainText;text:control.text;font:control.font;color:control.enabled?Theme.text:Theme.faint;opacity:control.enabled?1:.55;elide:Text.ElideRight;verticalAlignment:Text.AlignVCenter}
    background:Rectangle{radius:Theme.unit(9);color:Theme.selected;opacity:Math.max(0,Math.min(1,highlight.value));Motion{id:highlight;target:control.highlighted||control.down?1:0;epsilon:.002;enabled:Theme.motion&&control.visible}}
    Icon{x:Theme.unit(11);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(17);height:width;name:control.glyph;color:control.enabled?Theme.muted:Theme.faint;visible:control.glyph!==""}
    arrow:Icon{width:Theme.unit(13);height:width;x:control.width-width-Theme.unit(10);anchors.verticalCenter:parent.verticalCenter;name:"chevron-right";color:Theme.muted;visible:!!control.subMenu}
    indicator:Rectangle{x:Theme.unit(4);anchors.verticalCenter:parent.verticalCenter;width:Theme.unit(3);height:Theme.unit(14);radius:width/2;color:Theme.accent;visible:control.checkable&&control.checked}
}
