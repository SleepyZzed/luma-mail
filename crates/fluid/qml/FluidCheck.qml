import QtQuick
import QtQuick.Controls
CheckBox {
    id:control;implicitHeight:Theme.unit(32);spacing:Theme.unit(9);leftPadding:0;rightPadding:Theme.unit(8);hoverEnabled:true
    indicator:Rectangle{width:Theme.unit(17);height:width;y:(control.height-height)/2;radius:Theme.unit(5);color:control.checked?Theme.accent:Theme.surface;border.width:1;border.color:control.visualFocus?Theme.accent:control.checked?Theme.accent:Theme.muted
        Icon{anchors.centerIn:parent;width:Theme.unit(12);height:width;name:"check";color:Theme.accentText;visible:control.checked}
    }
    contentItem:Text{textFormat:Text.PlainText;text:control.text;elide:Text.ElideRight;color:control.enabled?Theme.text:Theme.faint;font.pixelSize:Theme.unit(11);verticalAlignment:Text.AlignVCenter;leftPadding:control.indicator.width+control.spacing}
}
